use modules::health::health_module::HealthModule;

mod modules;



async fn main() {
    let config = ConfigProvider::load_config();
    let mut app = wrapp::Application::builder();

    app.register_provider(config) // registers an existing thing modules can depend on
        .register_module(HealthModule) // registers modules which will then be instanciated by the app;
        .register_module(HealthModule::module().with_strategy_filter(|strat| strat.label.contains("private"))) // This module will only hook itself to strategies which fit to the predicate "private"
        .register_module(
            StrategyFiltered::filter(|strat| strat.label.contains("private"))
                .register_module(HealthModule::module())
        ); // This module will only hook itself to strategies which fit to the predicate

    if config.enable_web_server {
        app.register_strategy(
            WrappStrategies::ActixStrategy::builder()
            .use_global_middleware(actix_web::middleware::Logger::default()) 
                // Since this is a strategy made directly for actix, this should be able to use any Actix middleware
            .host("127.0.0.1")
            .tls(config.web.tls)
            .port(8080)
            .add_tag("public-api")
            .build()
        )      

        app.register_strategy(
            WrappStrategies::ActixStrategy::builder()
            .use_global_middleware(actix_web::middleware::Logger::default()) 
                // Since this is a strategy made directly for actix, this should be able to use any Actix middleware
            .host("127.0.0.1")
            .tls(config.web.tls)
            .port(8090)
            .add_tag("private-api")
            .build()
        )      
        // TODO - private/public api - api filters?  
    }

    if config.enable_kafka_consumer {
        app.register_strategy(
            WrappStrategies::KafkaStrategy::builder()
                .bootstrap_servers(&config.kafka.bootstrap_servers)
                .security_protocol(config.kafka.security_protocol)
                .sasl_mechanism(config.kafka.sasl_mechanism)
                .username(&config.kafka.username)
                .password(&config.kafka.password)
                .ssl_config(config.kafka.ssl_config)
                .add_tag("private-kafka")
                .build();
        )
        // TODO - we could register stategies with aliases - e.g. "private-web" "public-web" - thus we can more easily differenciate between different environments
        // Question would how to tell modules which alias to use? 
    }


    match app.run().await {
        Ok(()) => println!("Application ended without error code"),
        Err(e) => eprintln!("Application ended with error: {}", e),
    }

}


pub struct ConfigProvider;

impl ConfigProvider {
    fn load_config() -> ConfigProvider {
        ConfigProvider
    }
}


/*
 * Topics to think about
 * 
 * Strategy selection
 * 
 * - A web strategy might want to filter out 1. Modules 2. Endpoints inside the module
 * > Thus we require some kind of filtering.
 * 
 * IDEA 1 - Label based filtering 
 * 1. Label based filtering - add a label to the strategy
 * 2. During registration, a strategy could set up a filter for strategies
 * 3. The Module can also check the labels of the strategies it is being hooked to during the hooking function
 * 
 * IDEA 2 - Register Modules to the strategie directly?
 * 1. On the strategy builder, set whcih modules it should hook to
 * -> Disadvantage: Requires more code and more management of modules
 * 
 * 
 * Idea 3 - Label based filtering with App Scopes
 * - The app could be further divided into scopes.
 * - Each scope could have its own strategy set up or a label based strategy  filter
 * - However, that would need modules to register per scope they are in
 * -> Which would be confusing - as - does the module now exist once? Or multiple times?
 */
