use std::cmp::Ordering;

pub trait MinsBy: Iterator {
    fn mins_by<F>(self, mut f: F) -> Vec<Self::Item>
    where
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
        Self: Sized,
    {
        self.fold(vec![], |mut acc, item| {
            if let Some(x) = acc.first() {
                match f(x, &item) {
                    Ordering::Less => vec![item],
                    Ordering::Equal => {
                        acc.push(item);
                        acc
                    }
                    Ordering::Greater => acc,
                }
            } else {
                vec![item]
            }
        })
    }

    fn mins_by_key<F, B>(self, mut f: F) -> Vec<Self::Item>
    where
        F: FnMut(&Self::Item) -> B,
        B: Ord,
        Self: Sized,
    {
        let (acc, _) =
            self.map(|x| (f(&x), x))
                .fold((vec![], None), |(mut acc, min), (cmp, item)| {
                    let Some(min) = min else {
                        return (vec![item], Some(cmp));
                    };

                    match cmp.cmp(&min) {
                        Ordering::Less => (vec![item], Some(cmp)),
                        Ordering::Equal => {
                            acc.push(item);
                            (acc, Some(min))
                        }
                        Ordering::Greater => (acc, Some(min)),
                    }
                });

        acc
    }

    fn mins(self) -> Vec<Self::Item>
    where
        Self::Item: Ord,
        Self: Sized,
    {
        self.mins_by(Ord::cmp)
    }
}

impl<I: Iterator> MinsBy for I {}
