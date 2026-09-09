# fluorite-cast

*Ballistics Engine / Projectile Simulation for Godot 4, written in Rust*

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
		payload_scene, # Casts will carry this as the payload
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
	var origin = Transform3D(Basis.IDENTITY, Vector3(-5.0, 0.0, 0.0))
	var speed: float = 15.0
	var direction: Vector3 = Vector3(10.0, 2.0, 0.0).normalized()
	
	# Our will have this velocity
	var velocity: Vector3 = speed*direction;
	# We fire a cast on behalf of the factory
	factory.fire_cast(origin, velocity, {}, null, null)
```

## Installation

### From Precompiled Binary (Easiest)

Visit the [releases](https://github.com/StarlightSora/fluorite-cast/releases) page. *(TODO: More information regarding this goes here when a release is actually made)*

### For a Project Already Using [`godot-rust`](https://github.com/godot-rust/gdext)

Run `cargo add fluorite-cast` in your root crate. *(TODO: We don't have a crates.io release yet)*

Or alternatively, `git clone` this repository in your root crate, then edit your `Cargo.toml`'s `[dependencies]` section so it has this line:

```toml
fluorite-cast = { path = "fluorite-cast" }
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

*(TODO: GDExtension setup)*

## WIP

If you *really* want to try this out now, you need a Godot 4 project using godot-rust; clone this repository and move it into your root crate and use it as a dependency.