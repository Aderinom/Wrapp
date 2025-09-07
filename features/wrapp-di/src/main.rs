use std::{convert::Infallible, error::Error, fmt::Debug, sync::Arc};

use wrapp_di::{
    DependencyInfo, DiBuilder, DiHandle, InjectError, InstanceFactory, Lazy, LazyOption,
    ResolveStrategy,
};

fn main() {
    let app = DiBuilder::new()
        .add_instance(124 as i128)
        .add_instance("test".to_string())
        .add_factory(TestFactory);

    let app = futures::executor::block_on(app.build());
    let app = match app {
        Ok(app) => app,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    println!("{:?}", app);
    let t = app.require::<Test>();
    println!("{:?}", t);

    let a = Arc::new("String".to_owned());
    let b: &dyn Debug = &a;
}

#[derive(Debug)]
struct Test {
    a: Arc<String>,
    b: Lazy<String>,
    c: LazyOption<i128>,
}
struct TestFactory;
impl InstanceFactory for TestFactory {
    type Provides = Test;
    fn get_dependencies() -> Vec<DependencyInfo> {
        vec![
            Arc::<String>::dependency_info(),
            Lazy::<String>::dependency_info(),
            LazyOption::<i128>::dependency_info(),
        ]
    }

    async fn construct(
        &mut self,
        mut di: DiHandle,
    ) -> Result<Self::Provides, Box<dyn Error + Send + Sync>> {
        let str = Arc::<String>::resolve(&mut di).await?;
        let str2 = Lazy::<String>::resolve(&mut di).await?;
        let str3 = LazyOption::<i128>::resolve(&mut di).await?;
        Ok(Test {
            a: str,
            b: str2,
            c: str3,
        })
    }
}
