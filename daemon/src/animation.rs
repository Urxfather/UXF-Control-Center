pub trait HardwareAnimationBackend: Send + Sync {
    fn start_breathing(&self) -> Result<(), String>;
    fn start_rainbow(&self) -> Result<(), String> {
        Err("Rainbow animation is not supported by this backend".into())
    }
    fn stop_animation(&self) -> Result<(), String>;
    fn breathing_selected(&self) -> bool {
        false
    }
    fn rainbow_selected(&self) -> bool {
        false
    }
}
