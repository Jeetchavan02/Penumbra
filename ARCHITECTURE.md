# PENUMBRA architecture

Only the decisions that are expensive to reverse are recorded here.

## Module map

```
src/
  lib.rs      library root; main.rs is a thin CLI over it
  math/       Vec3, Ray, Interval, Sampler
  geometry/   Aabb, HitRecord, MaterialId, Hittable, Sphere, Quad, Triangle, Primitive
  material/   Scatter, Material, Lambertian, Metal, Dielectric, Emissive
  accel/      Bvh, SplitMethod
  mesh/       Mesh, OBJ reader and writer, procedural shapes
  camera/     CameraSpec, Camera
  scene/      Scene, SceneBuilder, Background, scene library
  render/     integrator (path loop), Film (accumulation, sRGB, PNG), render()
  main.rs     argument parsing and output
```

Dependencies point one way: `math <- geometry <- material`, `geometry <- accel`,
then `scene` owns all of them and `render` only sees `Scene`. `mesh` is an
authoring format: it is flattened into `Triangle` primitives when added to a
scene, so the renderer never sees an index buffer.

Changes from the original plan:

1. **Library plus binary.** Integration tests need to build a scene and render
   it; they cannot do that through `main.rs`.
2. **`MaterialId` lives in `geometry`, not `material`.** A hit record names its
   material by index. That keeps `geometry` from importing `material` and
   removes every lifetime from the hot path (see problem 2).
3. **`Interval` and `Sampler` live in `math`.** `rand` is imported in exactly
   one file, `math/sampler.rs`.
4. **`render` is split into integrator and film.** The path loop produces
   linear radiance; clamping, the sRGB curve and PNG encoding belong to
   `Film`. `image` is imported in exactly one file, `render/film.rs`.
5. **`scene` owns the BVH, the material table, the light list, the background
   and the camera description.** One value, built once, shared read-only.

## Key signatures

```rust
// math
pub struct Vec3 { pub x: f64, pub y: f64, pub z: f64 }    // Copy
pub type Point3 = Vec3;
pub type Color = Vec3;
pub struct Ray { origin: Point3, dir: Vec3, inv_dir: Vec3 } // dir is unit length
pub struct Interval { pub min: f64, pub max: f64 }
pub struct Sampler { /* seeded rand::SmallRng */ }
impl Sampler { pub fn for_pixel(seed: u64, pixel: u64) -> Sampler; }

// geometry
pub struct MaterialId(u32);
pub struct HitRecord { pub t: f64, pub point: Point3, pub normal: Vec3,
                       pub front_face: bool, pub material: MaterialId }
pub trait Hittable: Send + Sync {
    fn hit(&self, ray: &Ray, t: Interval) -> Option<HitRecord>;
    fn bounding_box(&self) -> Aabb;
}
pub enum Primitive { Sphere(Sphere), Quad(Quad), Triangle(Triangle) } // Copy, impl Hittable

// material
pub struct Scatter { pub attenuation: Color, pub ray: Ray, pub specular: bool }
pub enum Material { Lambertian(..), Metal(..), Dielectric(..), Emissive(..) }
impl Material {
    pub fn scatter(&self, ray_in: &Ray, hit: &HitRecord, sampler: &mut Sampler)
        -> Option<Scatter>;
    pub fn emitted(&self) -> Color;
}

// accel
pub enum SplitMethod { Midpoint, Sah }
pub struct Bvh { nodes: Vec<Node>, primitives: Vec<Primitive> } // impl Hittable

// scene
pub struct Scene { bvh: Bvh, materials: Vec<Material>, lights: Vec<Primitive>,
                   pub background: Background, pub camera: CameraSpec }

// render
pub fn radiance(scene: &Scene, ray: Ray, settings: &RenderSettings,
                sampler: &mut Sampler) -> Color;
pub fn render(scene: &Scene, settings: &RenderSettings) -> (Film, RenderStats);
```

## 1. Hittable: enum storage, trait as the interface

**Recommendation: `enum Primitive` stored by value in a flat `Vec`, with the
`Hittable` trait kept as the shared interface that `Sphere`, `Quad`,
`Primitive` and `Bvh` all implement.**

| | `Box<dyn Hittable>` | `enum Primitive` |
|---|---|---|
| Dispatch | indirect call, never inlined | `match`, inlined into the leaf loop |
| Memory | one heap allocation per object, pointer chase per test | contiguous, a leaf is a slice |
| BVH | nodes are themselves boxed trait objects, so the tree is a pointer graph | BVH is two flat vectors and indexes into one of them |
| Ownership | `Box` for tree, `Arc` once anything is shared | `Copy`; no ownership questions at all |
| `Send + Sync` | must be written on the trait or on every `Box<dyn ..>` | derived automatically |
| Adding a shape | new `impl` | new variant plus one arm in two `match`es |

The vtable call itself is not the real cost. The real costs are that the
pointer-graph BVH forces `Box`/`Arc` decisions into every constructor, and that
the BVH build wants to sort and partition primitives in place, which is trivial
on a `Vec<Primitive>` and awkward on a `Vec<Box<dyn Hittable>>`.

The enum's only weakness is that it is closed. For a renderer whose shape list
is known up front, that is a feature: the compiler tells you every place that
needs a new arm.

## 2. Material ownership

```rust
pub fn scatter(&self, ray_in: &Ray, hit: &HitRecord, sampler: &mut Sampler)
    -> Option<Scatter>;
```

- `&self`: materials are immutable shared data.
- `ray_in: &Ray`, `hit: &HitRecord`: read-only borrows. Both are `Copy`, so
  passing by value would also be fine; borrowing documents intent.
- `sampler: &mut Sampler`: the only mutable state, and it is owned by the
  calling thread's pixel loop, never by the scene.
- Returns an owned `Option<Scatter>`. `None` means absorbed. No out-parameters:
  the C++ `bool scatter(.., color& attenuation, ray& scattered)` shape has no
  reason to exist in Rust.

**Lifetime annotations required: none.** That is a consequence of
`HitRecord` holding a `MaterialId` instead of `&'a Material`. With a reference,
`HitRecord<'a>` infects `Hittable::hit`, the BVH traversal and the integrator,
and the bounce loop then holds a borrow of the scene across iterations while
also reassigning the ray. It compiles, but every signature carries `'a` for no
gain. An index lookup into `Vec<Material>` costs one bounds check per bounce.

The other tempting design, `Rc<dyn Material>` inside the hit record (the direct
translation of `shared_ptr<material>`), is worse: `Rc` is not `Send`, so it
blocks rayon entirely, and `Arc` adds an atomic increment per ray hit.

## 3. BVH: build both, default to SAH

Midpoint split costs nothing to write; binned SAH is about sixty more lines on
the same flat layout, so both are implemented behind `SplitMethod`.

What midpoint concretely loses on ~20 spheres: almost nothing. Brute force is
20 sphere tests per ray; any reasonable BVH is roughly eight box tests and two
or three sphere tests. SAH might remove one or two box tests. The scene is
dominated by shading and sampling, not traversal.

Where midpoint actually hurts:

- **Non-uniform density.** Midpoint splits space, not primitives. A cluster of
  small objects next to a few large ones produces lopsided trees; when every
  centroid lands on one side the build must fall back to a median split.
- **Mixed sizes.** A huge ground sphere overlaps every node it is in. SAH
  isolates it near the root because it prices the surface area; midpoint only
  does so by accident.

SAH starts to pay for itself around a thousand primitives and becomes essential
for triangle meshes. Measured numbers for this build are in the README.

Layout decisions that are hard to change later and are fixed now:

- Nodes in one `Vec`, depth-first, left child at `index + 1`. No `Box`, no
  recursion at traversal time, fixed-size stack.
- The BVH owns the primitives and reorders them, so a leaf is a slice.
- Each interior node stores its split axis so traversal visits the near child
  first, which lets the shrinking `t_max` cull the far child.
- Tree depth is bounded: below depth 32 the build switches to median splits.

## 4. Numerical stability: exact failure modes

**`t_min` epsilon.** Every ray direction is unit length (asserted in debug
builds), so `t` is a world-space distance and one epsilon has one meaning.
`RAY_EPSILON = 1e-4`.
- Too small (or zero): a scattered ray re-hits the surface it left. Lambertian
  surfaces get *shadow acne*, a speckle that looks like noise but never
  converges away.
- Too large: rays tunnel through geometry thinner than epsilon and miss contact
  points. Objects appear to float and light leaks through corners.
- Unnormalised directions make epsilon mean a different distance for every ray.
  This is the usual cause of acne that appears only on some materials.

**Self-intersection.** Besides `t_min`, sphere hit points are re-projected
onto the sphere (`center + normal * radius`) so the error does not grow with
distance from the camera. Without it, a radius-1000 ground sphere shows acne
far from the origin first.

**NaN propagation.** One NaN sample poisons a pixel's whole sum, and after
conversion it becomes a black (or white) dot that survives any sample count.
Sources:
- `normal + random_unit_vector` cancelling to zero, then normalising: `0/0`.
- `sqrt` of a slightly negative discriminant or `1 - cos^2`.
- `0 * inf` in the AABB slab test when a ray lies exactly in a slab plane.
- A camera whose `vup` is parallel to its view direction: the whole frame is
  NaN.

Defences: the Lambertian falls back to the normal when the sum is near zero;
every `sqrt` argument is clamped; the slab test is written with `f64::max/min`
so a NaN bound is ignored (conservative hit) rather than propagated; the camera
constructor panics on a degenerate basis; and the pixel loop rejects any
non-finite or negative sample and **counts** it in `RenderStats`, so a non-zero
count is visible instead of silent.

**Total internal reflection.**
- `cos_theta = (-d . n)` can exceed 1 by rounding; `sin_theta = sqrt(1 - cos^2)`
  then yields NaN. Clamp `cos_theta` to 1.
- The refraction ratio must flip on `front_face`. Getting it backwards still
  renders plausible glass; it is just the wrong glass (looks like a bubble).
- The TIR test (`ratio * sin_theta > 1`) must come before `refract`, whose
  square root is otherwise taken of a negative number.
- Hollow glass is modelled as an inner sphere of relative index `1/ior`, not a
  negative radius. Negative radii produce inverted bounding boxes.

**Others worth naming.** A metal with fuzz can scatter below the surface;
that ray must be absorbed, not traced. Quads have zero-thickness bounding
boxes on one axis and must be padded or the slab test rejects them.
Russian roulette must divide by the survival probability or the image darkens
with depth.

## 5. Parallelism

**Recommendation: `par_chunks_mut(width)` over the pixel buffer, one task per
scanline.**

A scanline is thousands of rays, so scheduling overhead is unmeasurable, and
rayon's work stealing already balances cheap rows against expensive ones.
Disjoint mutable rows fall straight out of `par_chunks_mut` with no `Mutex`
and no `unsafe`. Tiles only win when you want progressive display.

What rayon requires: the closure is `Fn + Send + Sync`, and it captures
`&Scene`, so **`Scene` must be `Sync`**. That is the whole constraint.

- With `Box<dyn Hittable>` you must opt in: `trait Hittable: Send + Sync`, or
  `Box<dyn Hittable + Send + Sync>` everywhere. Forget it and the error shows
  up at the `par_iter` call site, far from the cause.
- With enums of plain data, `Send + Sync` are derived. `Hittable` still names
  them as supertraits so any future implementor is forced to comply, and
  `scene/mod.rs` has a compile-time assertion that `Scene: Send + Sync`.

Random numbers are the second constraint. A shared generator needs a lock; the
thread-local one works but makes the image depend on thread scheduling. Each
pixel constructs its own `Sampler` from `hash(seed, pixel_index)`, so a render
is bit-identical for any thread count, and a bad pixel can be re-rendered alone.

## The three decisions most likely to cause silent wrong renders

1. **Unit-length ray directions as an invariant.** Everything about epsilon
   depends on it. Enforced in `Ray::new` (debug assertion).
2. **Where non-finite samples go.** Rejecting and counting them per sample,
   before accumulation, is the difference between one lost sample and a
   permanently dead pixel.
3. **`front_face` and the normal convention.** `HitRecord::normal` always
   opposes the incoming ray, and `front_face` records which side was hit.
   Every material depends on this, and a mistake still produces a
   plausible-looking image.

## The two decisions most likely to block parallelism later

1. **Any shared-ownership or interior-mutability type in the scene**: `Rc`,
   `RefCell`, `Cell` counters for statistics. The scene is immutable plain data
   after `build()`; statistics live in atomics owned by the render loop.
2. **Random state that is not owned by the pixel.** A generator threaded
   through the scene, or stored in a material or the camera, forces `&mut` or
   a lock on the shared data.

## What to build first to validate the architecture

A furnace test, end to end, on all cores: one sphere with albedo 0.8 in a
uniform white environment, rendered through `Scene`, `Bvh`, `Material`,
`render()` and rayon. Every pixel must be exactly 1.0 (miss) or exactly 0.8
(hit). It exercises every module boundary above, it fails loudly on energy
loss, acne and NaN, and it has a known answer. It is `tests/furnace.rs`.
