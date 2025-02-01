use std::{ops::Deref, sync::Arc};



type HealthCheck = dyn Fn() -> Result<(), Box<dyn std::error::Error>>;


pub struct HealthService(Arc<HealthServiceInner>);
struct HealthServiceInner {
    health_checks: Vec<Box<HealthCheck>>
}

impl Deref for HealthService {
    type Target = Arc<HealthServiceInner>;
    fn deref(&self) -> &Arc<HealthServiceInner> {
        &self.0
    }
}

impl HealthServiceInner {
    fn register_health_check(&mut self, check: Box<HealthCheck>) {
        self.health_checks.push(check);
    }

    fn check_health(&self) -> Result<(), Box<dyn std::error::Error>> {
        for check in &self.health_checks {
            if let Err(e) = check() {
                return Err(e);
            }
        }
        Ok(())
    }
}
