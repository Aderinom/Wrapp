use wrapp_config::provider::ConfigProvider;

#[derive(Clone)]
struct AppConfig {
    host: String,
    port: u16,
    app_name: String,
}

fn main() {
    let app_config = AppConfig {
        host: "localhost".to_string(),
        port: 8080_u16,
        app_name: "My Awesome App".to_string(),
    };

    let mut config_provider = ConfigProvider::new();
    let config_provider = match config_provider.add_config(app_config.clone()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e:?}");
            return;
        }
    };

    let retrieved_config = match config_provider.get_config::<AppConfig>() {
        Ok(Some(c)) => c,
        Ok(None) => {
            eprintln!("Could not find config type");
            return;
        }
        Err(e) => {
            eprintln!("{e:?}");
            return;
        }
    };

    assert_eq!(app_config.host, retrieved_config.host);
    assert_eq!(app_config.port, retrieved_config.port);
    assert_eq!(app_config.app_name, retrieved_config.app_name);
}
