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
    #[var]
    pub custom_data: VarDictionary,
    // TODO: Implement gravity effect multiplier
    // TODO: start worrying about acutal ray/shapecasting soon
}

#[godot_api]
impl FluoriteCastConfig {
    #[func]
    pub fn new_config(
        &parent_to: Gd<Node3D>, // probably don't want to move this
        max_alive_time: f64,
        max_total_length: f64,
        evaluate_mode: EvaluateMode,
        super_sampling_mode: SuperSamplingMode,
        target_delta: f64,
        target_length: f64,
        max_supersampling: i64,
        &custom_data: VarDictionary, // probably don't want to move this either
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                // TODO: Resources cannot hold direct references to Nodes in GDScript. I have no idea why this isn't raising an error. This might crash.
                // So this probably should be either stored as a NodePath instead, or have the base of this struct be Base<Node3D> instead
                // IDEA 1: Once FluoriteCastFactory is starting to be worked on, just let it accept the path and have it resolve to an actual node reference, since that struct is Base<Node3D>
                // IDEA 2: Scrap this field and have the caller assign the node into FluoriteCastFactory directly instead
                parent_to: Some(parent_to.clone()),
                max_total_length,
                max_alive_time,
                evaluate_mode,
                super_sampling_mode,
                target_delta,
                target_length,
                max_supersampling,
                custom_data: custom_data.clone(),
            }
        })
    }
    #[func]
    pub fn new_default_config(&parent_to: Gd<Node3D>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                parent_to: Some(parent_to.clone()),
                max_total_length: 2000.0,
                max_alive_time: 15.0,
                evaluate_mode: EvaluateMode::default(),
                super_sampling_mode: SuperSamplingMode::default(),
                target_delta: Self::hz_to_delta(60.0 * 0.95),
                target_length: 25.0,
                max_supersampling: 4,
                custom_data: VarDictionary::new(),
            }
        })
    }
    #[func]
    pub fn hz_to_delta(hz: f64) -> f64 {
        1.0 / hz
    }
}
