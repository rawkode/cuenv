//! Functional composition utilities for enhanced code readability and maintainability

use std::fmt::Debug;
use std::future::Future;

/// Function composition trait for creating pipelines
pub trait Compose<A, B> {
    /// Compose this function with another function
    fn compose<F, C>(self, f: F) -> impl Fn(A) -> C
    where
        F: Fn(B) -> C,
        Self: Fn(A) -> B + Sized;

    /// Pipe this function into another function (reverse composition)
    fn pipe<F, C>(self, f: F) -> impl Fn(A) -> C
    where
        F: Fn(B) -> C,
        Self: Fn(A) -> B + Sized;
}

impl<T, A, B> Compose<A, B> for T
where
    T: Fn(A) -> B,
{
    fn compose<F, C>(self, f: F) -> impl Fn(A) -> C
    where
        F: Fn(B) -> C,
    {
        move |a| f(self(a))
    }

    fn pipe<F, C>(self, f: F) -> impl Fn(A) -> C
    where
        F: Fn(B) -> C,
    {
        move |a| f(self(a))
    }
}

/// Pipe operator for functional composition
pub struct Pipe<T>(pub T);

impl<T> Pipe<T> {
    /// Create a new pipe
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Apply a function to the wrapped value
    pub fn pipe<F, U>(self, f: F) -> Pipe<U>
    where
        F: FnOnce(T) -> U,
    {
        Pipe(f(self.0))
    }

    /// Apply a fallible function to the wrapped value
    pub fn try_pipe<F, U, E>(self, f: F) -> Result<Pipe<U>, E>
    where
        F: FnOnce(T) -> Result<U, E>,
    {
        f(self.0).map(Pipe)
    }

    /// Apply a function only if condition is true
    pub fn pipe_if<F>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(T) -> T,
    {
        if condition {
            Pipe(f(self.0))
        } else {
            self
        }
    }

    /// Apply a function based on a predicate
    pub fn pipe_when<P, F>(self, predicate: P, f: F) -> Self
    where
        P: FnOnce(&T) -> bool,
        F: FnOnce(T) -> T,
    {
        if predicate(&self.0) {
            Pipe(f(self.0))
        } else {
            self
        }
    }

    /// Extract the wrapped value
    pub fn into_inner(self) -> T {
        self.0
    }

    /// Get a reference to the wrapped value
    pub fn inner(&self) -> &T {
        &self.0
    }

    /// Apply a side effect function without changing the value
    pub fn tap<F>(self, f: F) -> Self
    where
        F: FnOnce(&T),
    {
        f(&self.0);
        self
    }

    /// Apply an async function to the wrapped value
    pub async fn pipe_async<F, Fut, U>(self, f: F) -> Pipe<U>
    where
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = U>,
    {
        Pipe(f(self.0).await)
    }

    /// Apply a fallible async function to the wrapped value
    pub async fn try_pipe_async<F, Fut, U, E>(self, f: F) -> Result<Pipe<U>, E>
    where
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = Result<U, E>>,
    {
        f(self.0).await.map(Pipe)
    }
}

impl<T: Debug> Debug for Pipe<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pipe({:?})", self.0)
    }
}

impl<T: Clone> Clone for Pipe<T> {
    fn clone(&self) -> Self {
        Pipe(self.0.clone())
    }
}

impl<T> From<T> for Pipe<T> {
    fn from(value: T) -> Self {
        Pipe(value)
    }
}

/// Function composition operators
pub mod operators {

    /// Forward composition operator (f >> g)
    pub fn forward_compose<A, B, C, F, G>(f: F, g: G) -> impl Fn(A) -> C
    where
        F: Fn(A) -> B,
        G: Fn(B) -> C,
    {
        move |a| g(f(a))
    }

    /// Backward composition operator (g << f)
    pub fn backward_compose<A, B, C, F, G>(g: G, f: F) -> impl Fn(A) -> C
    where
        F: Fn(A) -> B,
        G: Fn(B) -> C,
    {
        move |a| g(f(a))
    }

    /// Identity function
    pub fn identity<T>(x: T) -> T {
        x
    }

    /// Constant function
    pub fn constant<T, U>(value: T) -> impl Fn(U) -> T
    where
        T: Clone,
    {
        move |_| value.clone()
    }

    /// Flip the arguments of a two-argument function
    pub fn flip<A, B, C, F>(f: F) -> impl Fn(B, A) -> C
    where
        F: Fn(A, B) -> C,
    {
        move |b, a| f(a, b)
    }

    /// Curry a two-argument function
    pub fn curry<A, B, C, F>(f: F) -> impl Fn(A) -> Box<dyn Fn(B) -> C>
    where
        F: Fn(A, B) -> C + Clone + 'static,
        A: Clone + 'static,
        B: 'static,
        C: 'static,
    {
        move |a| {
            let f = f.clone();
            let a = a.clone();
            Box::new(move |b| f(a.clone(), b))
        }
    }

    /// Uncurry a curried function
    pub fn uncurry<A, B, C, F>(f: F) -> impl Fn(A, B) -> C
    where
        F: Fn(A) -> Box<dyn Fn(B) -> C>,
    {
        move |a, b| f(a)(b)
    }
}

/// Pipeline macro for more readable function composition
#[macro_export]
macro_rules! pipeline {
    ($value:expr) => {
        $crate::functional::composition::Pipe::new($value)
    };
    ($value:expr, $($func:expr),+ $(,)?) => {{
        let mut result = $crate::functional::composition::Pipe::new($value);
        $(
            result = result.pipe($func);
        )+
        result
    }};
}

/// Try pipeline macro for fallible operations
#[macro_export]
macro_rules! try_pipeline {
    ($value:expr) => {
        Ok($crate::functional::composition::Pipe::new($value))
    };
    ($value:expr, $($func:expr),+ $(,)?) => {{
        let mut result = $crate::functional::composition::Pipe::new($value);
        $(
            result = result.try_pipe($func)?;
        )+
        Ok(result)
    }};
}

/// Async pipeline macro
#[macro_export]
macro_rules! async_pipeline {
    ($value:expr) => {
        async move { $crate::functional::composition::Pipe::new($value) }
    };
    ($value:expr, $($func:expr),+ $(,)?) => {
        async move {
            let mut result = $crate::functional::composition::Pipe::new($value);
            $(
                result = result.pipe_async($func).await;
            )+
            result
        }
    };
}

/// Functional utilities for working with Options
pub trait OptionExt<T> {
    /// Apply a function if Some, otherwise use default
    fn map_or_else_with<U, F, D>(self, default: D, f: F) -> U
    where
        F: FnOnce(T) -> U,
        D: FnOnce() -> U;

    /// Chain multiple option-returning functions
    fn chain<U, F>(self, f: F) -> Option<U>
    where
        F: FnOnce(T) -> Option<U>;

    /// Tap into Some values for side effects
    fn tap_some<F>(self, f: F) -> Self
    where
        F: FnOnce(&T);

    /// Tap into None for side effects
    fn tap_none<F>(self, f: F) -> Self
    where
        F: FnOnce();
}

impl<T> OptionExt<T> for Option<T> {
    fn map_or_else_with<U, F, D>(self, default: D, f: F) -> U
    where
        F: FnOnce(T) -> U,
        D: FnOnce() -> U,
    {
        match self {
            Some(value) => f(value),
            None => default(),
        }
    }

    fn chain<U, F>(self, f: F) -> Option<U>
    where
        F: FnOnce(T) -> Option<U>,
    {
        self.and_then(f)
    }

    fn tap_some<F>(self, f: F) -> Self
    where
        F: FnOnce(&T),
    {
        if let Some(ref value) = self {
            f(value);
        }
        self
    }

    fn tap_none<F>(self, f: F) -> Self
    where
        F: FnOnce(),
    {
        if self.is_none() {
            f();
        }
        self
    }
}

/// Functional utilities for working with iterators
pub trait IteratorExt<T>: Iterator<Item = T> + Sized {
    /// Apply a function to each element and collect results
    fn map_collect<U, F, C>(self, f: F) -> C
    where
        F: FnMut(T) -> U,
        C: FromIterator<U>,
    {
        self.map(f).collect()
    }

    /// Filter and map in one operation
    fn filter_map_collect<U, F, C>(self, f: F) -> C
    where
        F: FnMut(T) -> Option<U>,
        C: FromIterator<U>,
    {
        self.filter_map(f).collect()
    }

    /// Tap into each element for side effects
    fn tap_each<F>(self, f: F) -> std::iter::Inspect<Self, impl FnMut(&T)>
    where
        F: Fn(&T) + Clone,
    {
        self.inspect(move |item| {
            f(item);
        })
    }

    /// Partition into two collections based on a predicate
    fn partition_collect<P>(self, predicate: P) -> (Vec<T>, Vec<T>)
    where
        P: FnMut(&T) -> bool,
    {
        self.partition(predicate)
    }
}

impl<T, I> IteratorExt<T> for I where I: Iterator<Item = T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe_basic() {
        let result = Pipe::new(5).pipe(|x| x * 2).pipe(|x| x + 3).into_inner();

        assert_eq!(result, 13);
    }

    #[test]
    fn test_pipe_conditional() {
        let result1 = Pipe::new(10).pipe_if(true, |x| x * 2).into_inner();

        let result2 = Pipe::new(10).pipe_if(false, |x| x * 2).into_inner();

        assert_eq!(result1, 20);
        assert_eq!(result2, 10);
    }

    #[test]
    fn test_pipe_when() {
        let result = Pipe::new(15).pipe_when(|&x| x > 10, |x| x * 3).into_inner();

        assert_eq!(result, 45);
    }

    #[test]
    fn test_composition_operators() {
        use operators::*;

        let add_one = |x: i32| x + 1;
        let multiply_two = |x: i32| x * 2;

        let composed = forward_compose(add_one, multiply_two);
        assert_eq!(composed(5), 12); // (5 + 1) * 2
    }

    #[test]
    fn test_pipeline_macro() {
        let result = pipeline!(10, |x| x + 5, |x| x * 2, |x| x - 3).into_inner();

        assert_eq!(result, 27); // ((10 + 5) * 2) - 3
    }

    #[test]
    fn test_option_extensions() {
        let some_value = Some(42);
        let none_value: Option<i32> = None;

        let result1 = some_value.map_or_else_with(|| 0, |x| x * 2);
        let result2 = none_value.map_or_else_with(|| 0, |x| x * 2);

        assert_eq!(result1, 84);
        assert_eq!(result2, 0);
    }

    #[tokio::test]
    async fn test_async_pipeline() {
        let result = async_pipeline!(5, |x| async move { x + 1 }, |x| async move { x * 2 })
            .await
            .into_inner();

        assert_eq!(result, 12);
    }
}
