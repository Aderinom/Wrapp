use std::{convert::Infallible, error::Error, sync::Arc};

use wrapp_di::{
    DependencyInfo, DiBuilder, DiHandle, InjectError, InstanceFactory, Lazy, ResolveStrategy,
};

fn main() {
    let app = DiBuilder::new()
        .add_instance("test".to_string())
        .add_factory(TestFactory);

    let app = futures::executor::block_on(app.build()).unwrap();

    println!("{:?}", app);
    let t = app.require::<Test>().unwrap();
    println!("{:?}", t);

    println!("{:?}", t.c);
}

#[derive(Debug)]
struct Test {
    a: Arc<String>,
    b: Lazy<String>,
    c: Lazy<i128>,
}
struct TestFactory;
impl InstanceFactory for TestFactory {
    type Provides = Test;
    fn get_dependencies() -> Vec<DependencyInfo> {
        vec![
            Arc::<String>::dependency_info(),
            Lazy::<i128>::dependency_info(),
        ]
    }

    async fn construct(
        &mut self,
        mut di: DiHandle,
    ) -> Result<Self::Provides, Box<dyn Error + Send + Sync>> {
        let str = Arc::<String>::resolve(&mut di).await?;
        let str2 = Lazy::<String>::resolve(&mut di).await?;
        let str3 = Lazy::<i128>::resolve(&mut di).await?;
        Ok(Test {
            a: str,
            b: str2,
            c: str3,
        })
    }
}
