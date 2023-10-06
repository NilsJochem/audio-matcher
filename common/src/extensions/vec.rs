pub trait PushReturn {
    type Type;
    fn push_return(&mut self, t: Self::Type) -> &mut Self::Type;
}
impl<T> PushReturn for Vec<T> {
    type Type = T;
    fn push_return(&mut self, t: Self::Type) -> &mut Self::Type {
        self.push(t);
        self.last_mut().unwrap()
    }
}

pub trait FindOrPush {
    type Type;
    fn find_or_push(
        &mut self,
        default: Self::Type,
        predicate: impl FnMut(&Self::Type) -> bool,
    ) -> &mut Self::Type {
        self.find_or_push_else(|| default, predicate)
    }
    fn find_or_push_else(
        &mut self,
        default: impl FnOnce() -> Self::Type,
        predicate: impl FnMut(&Self::Type) -> bool,
    ) -> &mut Self::Type;
}
impl<T> FindOrPush for Vec<T> {
    type Type = T;
    fn find_or_push_else(
        &mut self,
        default: impl FnOnce() -> Self::Type,
        mut predicate: impl FnMut(&Self::Type) -> bool,
    ) -> &mut Self::Type {
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
