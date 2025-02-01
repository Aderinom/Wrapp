use std::{borrow::Cow};

use super::{health_actix_endpoints, health_service::HealthService};


type HealthCheck = dyn Fn() -> Result<(), Box<dyn std::error::Error>>;


pub struct HealthModule {
    pub service: HealthService // Modules export by  making use of normal visibility rules
}

#[wrapp::module(
    ActixModule
)]
impl HealthModule {
    #[wrapp::module(factory)]
    pub fn new(service: HealthService) -> Self {
        HealthModule { service }
    }
    
    #[wrapp::module(condition)]
    pub fn enable(config: HealthModuleConfig) -> HealthCheck {
        self.service.health_check()
    }



}


impl ActixModule for HealthModule {
    fn scope(&mut self) -> Cow<'_, str> {
        "health".into()
    }

    fn register(&mut self, app: &mut actix_web::web::ServiceConfig, strategy_context: &WrappActix::StrategyContext) {
        if !strategy_context.tags.contains("private" ) {
            return; // Skip registration if the strategy is not marked as private.
        }

        app.configure(health_actix_endpoints::register)
            .app_data(actix_web::web::Data::new(self.service.clone()));
    }
    
    fn openapi() 
}


trait ActixModule {
    // Scopes the Module to a specific scope in the application.
    fn scope(&mut self) -> Cow<'_, str>;
    
    // Configures the Module and adds its endpoints to the application.
    fn register(&mut self, app: &mut actix_web::web::ServiceConfig, strategy_context: &WrappActix::StrategyContext);

}