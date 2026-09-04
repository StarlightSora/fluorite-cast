use godot::{classes::Shape3D, prelude::*};

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

#[derive(GodotConvert, Var, Export, Default, Clone, Debug, Copy)]
#[godot(via = i64)]
pub enum GravityBehavior {
    Ignore,
    #[default]
    UseGlobalGravityCached,
    UseGlobalGravityRealTime,
    UseCurrentGravityRealTime,
}

#[derive(GodotClass)]
#[class(init, base=Resource)]
pub struct FluoriteCastConfig {
    base: Base<Resource>,
    #[export]
    pub shape: Option<Gd<Shape3D>>,
    #[export(flags_3d_physics)]
    pub collision_mask: u32,
    #[export]
    pub max_alive_time: f64,
    #[export]
    pub max_total_length: f64,
    #[export]
    pub gravity_behavior: GravityBehavior,
    #[export]
    pub gravity_multiplier: f64,
    #[export]
    pub evaluate_mode: EvaluateMode,
    #[export]
    pub super_sampling_mode: SuperSamplingMode,
    #[export]
    pub target_delta: f64,
    #[export]
    pub target_length: f64,
    #[export]
    pub max_supersampling: i64,
    #[export]
    pub custom_data: VarDictionary,
    // TODO: start worrying about acutal ray/shapecasting soon
}

#[godot_api]
impl FluoriteCastConfig {
    #[func]
    pub fn new_config(
        shape: Gd<Shape3D>,
        collision_mask: u32,
        max_alive_time: f64,
        max_total_length: f64,
        gravity_behavior: GravityBehavior,
        gravity_multiplier: f64,
        evaluate_mode: EvaluateMode,
        super_sampling_mode: SuperSamplingMode,
        target_delta: f64,
        target_length: f64,
        max_supersampling: i64,
        &custom_data: VarDictionary, // probably don't want to move this, so get a reference and clone it like a Rc
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                shape: Some(shape.clone()),
                collision_mask,
                max_alive_time,
                max_total_length,
                gravity_behavior,
                gravity_multiplier,
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
    pub fn new_default_config(shape: Gd<Shape3D>) -> Gd<Self> {
        Gd::from_init_fn(|base| {
            Self {
                base,
                shape: Some(shape.clone()),
                collision_mask: 0b1,
                max_alive_time: 15.0,
                max_total_length: 2000.0,
                gravity_behavior: GravityBehavior::default(),
                gravity_multiplier: 1.0,
                evaluate_mode: EvaluateMode::default(),
                super_sampling_mode: SuperSamplingMode::default(),
                target_delta: Self::hz_to_delta(120.0 * 0.95),
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
    #[func]
    pub fn delta_to_hz(delta: f64) -> f64 {
        1.0 / delta
    }
}
