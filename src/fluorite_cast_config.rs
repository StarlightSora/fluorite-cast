use godot::prelude::*;

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum EvaluateMode {
    #[default]
    PhysicsProcess,
    Process,
    Manual,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum LookBehavior {
    #[default]
    FollowVelocity, // TODO: Not in use yet
    Manual,
}

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum SuperSamplingMode {
    Never,
    #[default]
    IfAboveTargetDelta,
    IfTooLong,
    IfAboveTargetDeltaOrTooLong,
}

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastConfig {
    base: Base<Resource>,
    #[var]
    pub parent_to: Option<Gd<Node3D>>,
    #[var]
    pub max_alive_time: f64,
    #[var]
    pub max_total_length: f64,
    #[var]
    pub evaluate_mode: EvaluateMode,
    #[var]
    pub super_sampling_mode: SuperSamplingMode,
    #[var]
    pub target_delta: f64,
    #[var]
    pub target_length: f64,
    #[var]
    pub max_supersampling: i64,
    // TODO: Implement gravity effect multiplier
    // TODO: start worrying about acutal ray/shapecasting soon
}

#[godot_api]
impl FluoriteCastConfig {
    #[func]
    pub fn new_config(
        parent_to: Gd<Node3D>,
        max_alive_time: f64,
        max_total_length: f64,
        evaluate_mode: EvaluateMode,
        super_sampling_mode: SuperSamplingMode,
        target_delta: f64,
        target_length: f64,
        max_supersampling: i64,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                parent_to: Some(parent_to),
                max_total_length,
                max_alive_time,
                evaluate_mode,
                super_sampling_mode,
                target_delta,
                target_length,
                max_supersampling,
            }
        })
    }
    #[func]
    pub fn new_default_config(parent_to: Gd<Node3D>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                parent_to: Some(parent_to),
                max_total_length: 2000.0,
                max_alive_time: 15.0,
                evaluate_mode: EvaluateMode::default(),
                super_sampling_mode: SuperSamplingMode::default(),
                target_delta: Self::hz_to_delta(60.0 * 0.95),
                target_length: 25.0,
                max_supersampling: 4,
            }
        })
    }
    #[func]
    pub fn hz_to_delta(hz: f64) -> f64 {
        1.0 / hz
    }
}
