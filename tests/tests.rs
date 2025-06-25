use sfo_event::sfo_event;

#[async_trait::async_trait]
#[sfo_event]
trait TestTrait1: 'static + Sync + Send {
    async fn test(&self) -> Result<(), ()>;
    fn test2(&self) -> Result<(), ()>;
}


#[async_trait::async_trait]
#[sfo_event(emitter=TestEmitter)]
pub trait TestTrait2<T:'static + Send + Sync>: 'static + Sync + Send {
    async fn test(&self, t: &T) -> Result<(), ()>;
}

#[test]
fn test_trait() {
    let emitter = TestTrait1Emitter::new();
    // emitter.add_listener(TestTrait1Listener::new());
    emitter.test2();
}

