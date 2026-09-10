# fluorite-cast

*Ballistics Engine / Projectile Simulation for Godot 4, written in Rust*

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
		payload_scene, # Casts will carry this as the payload (nullable)
		cast_cfg, # Casts will behave like this
		global_fluid_cfg # The global air is configured like this
	)
# We have a looping, autostarted timer as a child of this node
@onready var timer = $Timer

func _ready() -> void:
	# Connect the signal from the timer to the _on_timer_timeout function
	timer.connect("timeout", _on_timer_timeout)


func _on_timer_timeout() -> void:
	# Cast will spawn from here
	var origin = Transform3D(Basis.IDENTITY, Vector3(-7.0, 0.0, 0.0))
	var speed: float = 15.0
	var direction: Vector3 = Vector3(15.0, 2.0, 0.0).normalized()
	
	# Our will have this velocity
	var velocity: Vector3 = speed*direction;
	# We fire a cast on behalf of the factory
	factory.fire_cast(origin, velocity, {}, null, null)
```

## Installation

### From Precompiled Binary (Recommended)

This is the recommended way for most cases.

Visit the [releases](https://github.com/StarlightSora/fluorite-cast/releases) page, and download the latest release. It should be named `fluorite-cast_(VERSION NUMBER).7z`.

Unzip the folder. You should get a folder named `fluorite-cast`. Move this folder to the `addons` folder of your project (if it doesn't exist, then make it).

Note that only Windows and Linux binaries are provided at the moment. If you need support for other platforms, you will need to build from source.

### For a Project Already Using [`godot-rust`](https://github.com/godot-rust/gdext)

This is recommended if you already have a project using `godot-rust`, or want to use the Rust-only features of this library for extra performance.

Run `cargo add fluorite-cast` in your root crate. *(TODO: We don't have a crates.io release yet)*

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

# License

This project is licensed under the MIT license.