use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    sync::Arc,
    thread::{self, sleep},
    time::Duration,
};

use futures::{stream::FuturesUnordered, StreamExt};
use futures_channel::{mpsc, oneshot};

use crate::{
    builder::DiBuilder,
    container::DiContainer,
    dependency_graph::{DependencyGraph, DependencyGraphErrors},
    errors::{InitError, InjectError, RequireError},
    resolver::Resolver,
    types::{DynError, Instance, TypeInfo},
};

/// Initiates the DiContainer
pub(crate) struct DiInitiator {
    request_rx: mpsc::Receiver<DiRequest>,
    request_tx: mpsc::Sender<DiRequest>,

    all_registered_type_ids: HashSet<TypeId>,

    /// Waiters for instance results
    instance_waiters: HashMap<TypeId, Vec<DiResponseSender<Instance>>>,
    /// Waiters for the final DiContainer
    container_waiters: Vec<DiResponseSender<DiContainer>>,

    /// All produced instances - None = Type is known but disabled
    instances: HashMap<TypeId, (TypeInfo, Option<Instance>)>,
}
impl DiInitiator {
    pub(crate) fn new() -> DiInitiator {
        let (injection_request_sender, injection_request_receiver) = mpsc::channel(10);
        DiInitiator {
            request_rx: injection_request_receiver,
            request_tx: injection_request_sender,
            all_registered_type_ids: HashSet::new(),
            instances: HashMap::new(),
            instance_waiters: HashMap::new(),
            container_waiters: Vec::new(),
        }
    }

    pub async fn initiate(
        mut self,
        blueprint: DiBuilder,
        timeout: Option<Duration>,
    ) -> Result<DiContainer, InitError> {
        // If we have a timeout - spawn a thread to signal once it's done
        let (timeout_tx, timeout_rx) = oneshot::channel::<()>();
        if let Some(timeout) = timeout {
            // We don't join the thread - it will just die after the timeout
            thread::spawn(move || {
                sleep(timeout);
                let _ = timeout_tx.send(());
            });
        };

        // Build and check Graph
        let graph = DependencyGraph::new(&blueprint).map_err(|error| DependencyGraphErrors {
            errors: vec![error],
        })?;

        graph.check()?;

        // Start initiating all Instances
        if let Err(e) = self.try_initiate(blueprint, timeout_rx).await {
            // On fail - inform all waiters
            let msg = Err(e.clone().into());
            for (_, waiters) in self.instance_waiters {
                for waiter in waiters.into_iter() {
                    let _ = waiter.send(msg.clone());
                }
            }

            let msg = Err(e.clone().into());
            for waiter in self.container_waiters {
                let _ = waiter.send(msg.clone());
            }

            return Err(e);
        }

        tracing::debug!("All");

        let container = DiContainer::new(self.instances, graph);
        for waiter in self.container_waiters {
            let _ = waiter.send(Ok(container.clone()));
        }

        Ok(container)
    }

    /// Starts all registered factories and waits for them to complete
    async fn try_initiate(
        &mut self,
        blueprint: DiBuilder,
        mut timeout: oneshot::Receiver<()>,
    ) -> Result<(), InitError> {
        let DiBuilder {
            registered_factories,
            registered_instances,
        } = blueprint;

        tracing::debug!(
            "Initializing application with {} factories and {} instances",
            registered_factories.len(),
            registered_instances.len()
        );

        // Add all pre build instances to the results

        for (info, instance) in registered_instances.into_iter() {
            self.all_registered_type_ids.insert(info.type_id);
            self.instances.insert(info.type_id, (info, Some(instance)));
        }

        // ###############################################
        // Begin instantiation of all factories
        let mut factory_futures = FuturesUnordered::new();
        for mut factory in registered_factories {
            self.all_registered_type_ids
                .insert(factory.supplies().type_id);
            let handle = self.get_handle();

            // Returns error if DI failed
            // Returns None if dependency is disabled
            let factory_future = async move {
                let supply_info = factory.supplies();
                let result = async {
                    // Check if the factory is enabled
                    match Box::into_pin(factory.is_enabled(handle.clone())).await? {
                        true => {
                            tracing::debug!("Factory for {} is enabled", supply_info.type_name)
                        }
                        false => {
                            tracing::debug!("Factory for {} is disabled", supply_info.type_name);
                            return Ok(None);
                        }
                    }

                    // Construct factory
                    let instance = Box::into_pin(factory.construct(handle.clone())).await?;

                    tracing::debug!("Constructed instance of {}", instance.info.type_name);
                    Ok::<_, DynError>(Some(instance))
                }
                .await;

                (supply_info, result)
            };

            factory_futures.push(factory_future);
        }

        // Start handling injection requests and wait for all factories to finish
        // let pin_set = pin!(set);
        let factory_count = factory_futures.len();

        loop {
            let factories_left = factory_futures.len();
            tracing::debug!(
                "Waiting for factories to finish [{factories_left} of {factory_count} complete]"
            );

            futures::select! {
                request = self.request_rx.select_next_some() => {
                    self.handle_injection_request(request);
                }
                result = factory_futures.next() => {
                    if self.handle_factory_result(result)? {
                        break;
                    }
                }
                _ = timeout => {
                    return Err(InitError::Timeout)
                }
            }
        }

        Ok(())
    }

    /// Handle the result of a factory future
    ///
    /// Returns true if complete
    fn handle_factory_result(
        &mut self,
        result: Option<(TypeInfo, Result<Option<Instance>, DynError>)>,
    ) -> Result<bool, InitError> {
        let (info, result) = match result {
            Some(result) => result,
            None => {
                // If no more tasks are left, exit the loop
                // all injection requests must now also be handled as nothing is left to be build
                debug_assert!(
                    self.instance_waiters.is_empty(),
                    "Not all waiters were satisfied"
                );
                return Ok(true);
            }
        };

        match result {
            Ok(instance) => {
                handle_created_instance(self, info, instance);
            }
            Err(err) => {
                // If one factory fails - abort DI
                return Err(InitError::FactoryFailed {
                    product: info.type_name,
                    error: Arc::new(err),
                });
            }
        };

        return Ok(false);

        fn handle_created_instance(
            this: &mut DiInitiator,
            info: TypeInfo,
            instance: Option<Instance>,
        ) -> () {
            let type_id = info.type_id;
            let type_name = info.type_name;
            // Add instance to results
            this.instances.insert(type_id, (info, instance.clone()));

            let message = match instance {
                Some(instance) => Ok(instance),
                None => Err(RequireError::TypeDisabled(type_name)),
            };

            // Inform all waiters with a result
            for waiter in this
                .instance_waiters
                .remove(&type_id)
                .into_iter()
                .flat_map(Vec::into_iter)
            {
                let _ = waiter.send(message.clone());
            }
        }
    }

    /// Get a handle to the DiInitiator
    ///
    /// The handle is only valid before and during Initiation.
    pub fn get_handle(&self) -> DiHandle {
        DiHandle {
            request_sender: self.request_tx.clone(),
        }
    }
}
// Injection Request handlers
impl DiInitiator {
    fn handle_injection_request(&mut self, request: DiRequest) {
        match request {
            DiRequest::Require {
                type_info,
                response_channel,
            } => {
                self.handle_instance_require(type_info, response_channel);
            }
            DiRequest::RequireApp { response_channel } => self.handle_app_require(response_channel),
        }
    }

    fn handle_instance_require(
        &mut self,
        info: TypeInfo,
        response_channel: DiResponseSender<Instance>,
    ) {
        // Check if the TypeId is registered
        if !self.all_registered_type_ids.contains(&info.type_id) {
            tracing::error!("Tried to require an unregistered type: {}", info.type_name);
            let _ = response_channel.send(Err(RequireError::TypeMissing(info.type_name)));
            return;
        }

        // Check if we already have a result
        match self.instances.get(&info.type_id) {
            Some((_, result)) => {
                let _ = match result {
                    Some(instance) => response_channel.send(Ok(instance.clone())),
                    None => response_channel.send(Err(RequireError::TypeDisabled(info.type_name))),
                };
                return;
            }
            None => {} // No result yet
        }

        // Otherwise add the request to the waiters list
        self.instance_waiters
            .entry(info.type_id)
            .or_default()
            .push(response_channel);
    }

    fn handle_app_require(&mut self, response_channel: DiResponseSender<DiContainer>) {
        self.container_waiters.push(response_channel);
    }
}

/// DI Handle for resolving dependencies and getting instances from the registry.
/// The DI Handle is only valid during instantiation of the Application.
/// Afterwards the DI Container can be used directly for dependency injection.
#[derive(Clone)]
pub struct DiHandle {
    pub request_sender: mpsc::Sender<DiRequest>,
}
impl DiHandle {
    pub async fn resolve<T: Resolver>(&mut self) -> Result<T, InjectError> {
        T::resolve(self).await
    }
}

pub type DiResponseSender<For> = oneshot::Sender<Result<For, RequireError>>;
pub type DiResponseReceiver<For> = oneshot::Receiver<Result<For, RequireError>>;

/// Requests between [DiHandle] and [DiInitiator]
pub enum DiRequest {
    /// Requires an instance of a specific type
    Require {
        type_info: TypeInfo,
        response_channel: DiResponseSender<Instance>,
    },
    /// Requires a reference to the [DiContainer] once it has been build
    RequireApp {
        response_channel: DiResponseSender<DiContainer>,
    },
}
