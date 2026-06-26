//! Wrapp DI consists of following components:
//!
//! 1. DiBuilder - for registering all factories and instances.
//! 2. DiInitiator - which executes all factories, and handles injection requests during their initialization.
//! 2.1 Injection Handles - which are valid during the AppInit Phase, and allow a component to request its dependencies.
//! 3. DIContainer - which is the final container that holds all created instances.
//!
//! General logic:
//! AppBuilder calls BUild
//!
//! AppGraph queries all factories for their dependencies and creates a graph.
//! AppGraph checks the graph for circular dependencies, and aborts DI if:
//! - Any required Dependencies are missing
//! - Any Circular Dependencies were found
//!
//! AppInitiator creates tasks for all factories.
//! Factories start asking for their dependencies.
//! Some Factory will not require a dependency (or just requires already existing instances), and as such will complete
//! After Completion, AppInitiator gets the result of the completed task.
//! AppInitiator then informs all factories which were waiting for this dependency.
//! Step by step all factories will complete
//! After All factories have completed, AppInitiator creates the final DIContainer.
//! AppInitiator publishes the DIcontainer through a channel to all dependencies.
//! DiContainer wil also be given to the caller of AppBuilder.build()
//!

pub mod builder;
pub mod container;
pub mod dependency_graph;
pub mod errors;
pub mod factories;
pub mod initiator;
pub mod resolver;
pub mod types;
