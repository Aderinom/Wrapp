// There is no opinionated way for the structuring of endpoints
// Also note - I'm just typing this stuff out of memory, so no gurantees on correctness



pub fn register(app: &mut actix_web::Config) {
    app.service(get_health_state);
}



/// Gets the health state of the application
#[actix_web::get("/health")]
fn get_health_state(health: web::Data<HealthService>) -> HealthState {
    health.check_health()
}