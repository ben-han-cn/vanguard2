pub(crate) mod mock_resolver;
mod nsas;
mod recursor;
mod roothint;
mod running_query;

pub(crate) use recursor::RecursiveResolver;
pub use recursor::Recursor;
