use std::{convert::Infallible, error::Error, sync::Arc};

use wrapp_di::{
    DependencyInfo, DiBuilder, DiHandle, InjectError, InstanceFactory, ResolveStrategy,
};

fn main() {
    let app = DiBuilder::new()
        .add_instance("test".to_string())
        .add_factory(TestFactory);

    let app = futures::executor::block_on(app.build()).unwrap();

    println!("{:?}", app);
    let t = app.require::<Test>().unwrap();
    println!("{:?}", t)
}

#[derive(Debug)]
struct Test {
    a: Arc<String>,
}
struct TestFactory;
impl InstanceFactory for TestFactory {
    type Provides = Test;
    fn get_dependencies() -> Vec<DependencyInfo> {
        vec![]
    }

    async fn construct(&mut self, mut di: DiHandle) -> Result<Self::Provides, Box<dyn Error>> {
        let str = Arc::<String>::resolve(&mut di).await?;
        Ok(Test { a: str })
    }
}
