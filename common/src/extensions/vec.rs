/// extentions to push an Element and returning an reference
pub trait PushReturn<Type> {
    /// pushes `t` and returns a reference to it
    fn push_return(&mut self, t: Type) -> &mut Type;
}
impl<T> PushReturn<T> for Vec<T> {
    fn push_return(&mut self, t: T) -> &mut T {
        self.push(t);
        self.last_mut().unwrap()
    }
}
/// extentions to push an Element when a condition is met
pub trait FindOrPush<Type> {
    /// tries to find a element matching `predicate` or pushing `default`
    fn find_or_push(&mut self, default: Type, predicate: impl FnMut(&Type) -> bool) -> &mut Type {
        self.find_or_push_else(|| default, predicate)
    }
    /// tries to find a element matching `predicate` or pushing the result of `default`
    fn find_or_push_else(
        &mut self,
        default: impl FnOnce() -> Type,
        predicate: impl FnMut(&Type) -> bool,
    ) -> &mut Type;
}
impl<T> FindOrPush<T> for Vec<T> {
    fn find_or_push_else(
        &mut self,
        default: impl FnOnce() -> T,
        mut predicate: impl FnMut(&T) -> bool,
    ) -> &mut T {
        let index = self
            .iter_mut()
            .position(|t| predicate(t))
            .unwrap_or_else(|| {
                self.push(default());
                self.len() - 1
            });
        &mut self[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_or_push() {
        let mut data = vec![1, 2, 4];
        let element = data.find_or_push(0, |it| *it == 1);
        assert_eq!(1, *element, "get correct");
        *element = 7;
        assert!(data.iter().eq(&[7, 2, 4]), "first element got changed");
    }

    #[test]
    fn find_or_push_non_exiting() {
        let mut data = vec![1, 2, 4];

        let element = data.find_or_push(0, |&it| it == 3);
        assert_eq!(0, *element, "get correct");
        *element = 8;
        assert!(data.iter().eq(&[1, 2, 4, 8]), "first element got changed");
    }
}
