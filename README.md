# fluorite-cast

![crates_io](https://img.shields.io/crates/v/fluorite-cast)
![docs](https://img.shields.io/docsrs/fluorite-cast)
![lstcmt](https://img.shields.io/github/last-commit/StarlightSora/fluorite-cast)

*Ballistics Engine / Projectile Casting Simulation for Godot 4, written in Rust*

crates.io Disclaimer: **This crate is designed for use in Godot** (either as a precompiled binary or as a dependency of a project using `godot-rust`). *It is not useful as-is!*

## Why fluorite-cast?

Sometimes we make games that need **ranged hit detection**. Like a *FPS game* for example. We can use raycasting (or shapecasting) to do this. But that is **hitscan**; projectiles travel from the origin to the target instantly. But what if we want **projectiles that take time to travel, and a proper trajectory?**

Using physics bodies can work, but it's not good enough if we need reliable and performant projectiles. **`fluorite-cast` sidesteps the physics engine, using casting and mathematical evaluations per frame for reliability and performance.**

For more information please see the Features section.

## Example

```gdscript
extends Node3D
# Our casts will carry an instance of this scene
@onready var payload_scene: PackedScene = load("uid://c8bxc5h1nr5em")
# Our casts will behave like this
@onready var cast_cfg: FluoriteCastConfig = load("uid://wec84sfwltaa")
# The global air is configured like this
@onready var global_fluid_cfg: FluoriteFluidConfig = load("uid://bl5hugw7tksmq")
# This is our factory instance, which will make and fire casts
@onready var factory: FluoriteCastFactory = FluoriteCastFactory.new_factory(
		self, # Casts will be parented to this node
		global_fluid_cfg, # The global air is configured like this
		2, # The factory will orchestrate `evaluate` calls for each cast made every `physics_process`
		# (we can't use enums exported from Rust with their semantic names when calling functions, so we pass it as int)
		payload_scene, # Casts will carry this as the payload (nullable)
	)
# We have a looping, autostarted timer as a child of this node
@onready var timer = $Timer

func _ready() -> void:
	# We add the factory node to the scene tree
	self.add_child(factory)
	# Connect the timeout signal from the timer to _on_timer_timeout
	timer.timeout.connect(_on_timer_timeout)
	# Connect the terminated signal from the factory to _on_factory_cast_terminated
	factory.terminated.connect(_on_factory_cast_terminated)


func _on_timer_timeout() -> void:
	# Cast will spawn from here
	var origin = Transform3D(Basis.IDENTITY, Vector3(-7.0, 0.0, 0.0))
	var speed: float = 15.0
	var direction: Vector3 = Vector3(15.0, 2.0, 0.0).normalized()
	
	# Our will have this velocity
	var velocity: Vector3 = speed*direction;
	# We fire a cast on behalf of the factory
	factory.fire_cast(origin, velocity, cast_cfg, {}, null)

func _on_factory_cast_terminated(cast_instance: FluoriteCast, cast_result: FluoriteSpaceCastResult) -> void:
	print(cast_instance.name, " has terminated! Collider: ", cast_result.collider.name)
```

## Features

**Most features can be tuned down or turned off to suit your needs and to improve performance!**

- `FluoriteCast` Supports both **raycast** and **shapecast**-based casts (projectiles)

- `Area3D`s can influence **gravitational acceleration** on casts

- Casts can experience **drag** with fluid dynamics approximation, with `FluoriteFluidArea3D`s being able to override the global fluid

- **Supersampling** settings lets casts evaluate at higher precision

- A **factory type** `FluoriteCastFactory` that once constructed with configurations, can instantiate new `FluoriteCast`s with `fire_cast` calls, and forwards all signals emitted by casts it constructed

- **Custom callbacks** that can run when projectiles attempt to penetrate an object (`try_penetrate`), every time they get evaluated (`cast_raw_evaluated`), and right before they finish being instantiate (`on_new_cast`)

- **Signals** that fire when a cast penetrates (`penetrated`), terminates (`terminated`) and expires (`expired`)

- `FluoriteCast` extends `Node3D`, so it will **not unexpectedly push physics objects around!**

- Being purely written in Rust, a compiled systems programming language, it is **very performant**\*, with 1000+ casts with default configurations being simulated at once still keeping the game over 60FPS

- **No Rust knowledge is necessary**, all the features are still within reach of GDScript!

- ...But if you *do* use [`godot-rust`](https://github.com/godot-rust/gdext), you can use the library with Rust code, with access to Rust-optimized methods for extra performance!

<sup>*Assuming it was compiled as `cargo build --release`. Debug builds have suboptimal performance (around 50x slower with no optimizations). All precompiled binaries are shipped as release builds.</sup>

## Documentation

Please refer to the documentation on [docs.rs](https://docs.rs/fluorite-cast). Although the documentation is for Rust, it is still very relevant for GDScript usage.

From 0.1.1, the releases in the [releases](https://github.com/StarlightSora/fluorite-cast/releases) page now also ship with documentations generated with `cargo doc --no-deps` if you need documentation offline. Once you downloaded and unzipped it, please navigate to `fluorite_cast/index.html` to view the documentation. Note that dependencies of this crate are undocumented in the offline release.

## Installation

### From Precompiled Binary (Recommended)

This is the recommended way for most cases.

Visit the [releases](https://github.com/StarlightSora/fluorite-cast/releases) page, and download the latest release. It should be named `fluorite-cast_(VERSION NUMBER).7z`.

Unzip the folder. You should get a folder named `fluorite-cast`. Move this folder to the `addons` folder of your project (if it doesn't exist, then make it).

Note that only Windows and Linux binaries are provided at the moment. If you need support for other platforms, you will need to build from source.

### For a Project Already Using [`godot-rust`](https://github.com/godot-rust/gdext)

This is recommended if you already have a project using `godot-rust`, or want to use the Rust-only features of this library for extra performance.

Run `cargo add fluorite-cast` in your root crate.

Or alternatively, `git clone` this repository in your root crate, then edit your `Cargo.toml`'s `[dependencies]` section so it has this line:

```toml
[dependencies]
# ...
fluorite-cast = { path = "fluorite-cast" }
# ...
```

Then make sure to add this to your root crate's `lib.rs`:

```rust
extern crate fluorite_cast;
```

Finally run `cargo build` to rebuild your crate with the newly added dependency.

### Building From Source

Make sure you have [rustup](https://rustup.rs/) installed.

`git clone` this repository. Then, uncomment these lines in the `Cargo.toml` file:

```toml
## Uncomment below two lines if building a standalone cdylib build
#[lib]
#crate-type = ["cdylib"]
```

Now run `cargo build`. The built dynamic library file should be generated in `target/debug/`.

You'll have to set up the `.gdextension` file as well for Godot to recognize the dynamic library file. This is explained in more detail in the [godot-rust book](https://godot-rust.github.io/book/intro/hello-world.html#wire-up-godot-with-rust).

The `entry_symbol` of this library is `fluorite_cast`.

## Changelogs

Note: The API is not fully stable until it is bumped to `1.0.0`, a minor version bump (`0.x.y -> 0.x+1.y`) may introduce breaking changes!

- `0.2.0`: `FluoriteCastFactory`'s fields and constructor arguments changed to hand off the config resource to be injected per-cast to encourage end user scalability; having a default config in the factory proved to be cumbersome past demos

- `0.1.1`: Initial crates.io release

## License

This project is licensed under the MIT license.