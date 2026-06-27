//! An example of how a manual DI implementation can be done using the Wrapp DI framework.
//!
//! Usually you should use the wrapp-di procmacros to generate the factory for you,
//! but this example shows how to do it manually to help understanding what happens under the hood.
#![allow(dead_code)]

use std::{fmt::Debug, sync::Arc};

use wrapp_di::{
    builder::DiBuilder,
    errors::InjectError,
    factories::InstanceFactory,
    initiator::DiHandle,
    resolver::{
        Resolver,
        lazy::{Lazy, LazyOption},
    },
    types::DependencyInfo,
};

struct TestConfig {
    enable: bool,
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Create a new DI builder and register instances and factories
    let app = DiBuilder::new()
        .add_instance(TestConfig { enable: true })
        .add_instance(124_i128)
        .add_instance("test".to_string())
        .add_factory(TestFactory);

    // Build the DI container and handle any errors that may occur
    let app = futures::executor::block_on(app.build());
    let app = match app {
        Ok(app) => app,
        Err(e) => {
            println!("{e}");
            return;
        }
    };

    println!("{app:?}");
    let test_instance = app.require::<Test>();
    println!("{test_instance:?}");
}

/// Test struct requiring an Arc<String>, a Lazy<String> and a `LazyOption`<i128> as dependencies.
#[derive(Debug)]
struct Test {
    a: Arc<String>,
    b: Lazy<String>,
    c: LazyOption<i128>,
}
struct TestFactory;
impl InstanceFactory for TestFactory {
    type Provides = Test;

    /// Defines the dependencies required by the `TestFactory`.
    fn dependencies() -> Vec<DependencyInfo> {
        vec![
            Arc::<String>::dependency_info(),
            Lazy::<String>::dependency_info(),
            LazyOption::<i128>::dependency_info(),
        ]
    }

    /// Determines if the `TestFactory` is enabled based on the `TestConfig` instance in the DI
    /// container.
    ///
    /// This is run before the `construct` method and allows for conditional instantiation of the
    /// `Test` struct.
    async fn is_enabled(
        &mut self,
        mut di: DiHandle,
    ) -> Result<bool, impl Into<wrapp_di::types::DynError>> {
        let enabled = Option::<Arc<TestConfig>>::resolve(&mut di)
            .await?
            .is_some_and(|config| config.enable);
        Ok::<_, InjectError>(enabled)
    }

    /// Constructs a new instance of the Test struct with dependencies injected from the DI
    /// container.
    async fn construct(
        &mut self,
        mut di: DiHandle,
    ) -> Result<Self::Provides, impl Into<wrapp_di::types::DynError>> {
        // Resolve the dependencies from the DI container
        let str = Arc::<String>::resolve(&mut di).await?;
        let str2 = Lazy::<String>::resolve(&mut di).await?;
        let str3 = LazyOption::<i128>::resolve(&mut di).await?;

        // Return a new instance of Test with the resolved dependencies
        Ok::<_, InjectError>(Test {
            a: str,
            b: str2,
            c: str3,
        })
    }
}
