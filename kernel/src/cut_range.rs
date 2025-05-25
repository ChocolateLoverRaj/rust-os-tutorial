use core::ops::RangeInclusive;

pub trait CutRange<T> {
    fn cut(self, cut_out: &T) -> impl Iterator<Item = T>;
}

impl CutRange<RangeInclusive<u64>> for RangeInclusive<u64> {
    fn cut(self, cut_out: &RangeInclusive<u64>) -> impl Iterator<Item = RangeInclusive<u64>> {
        let result: heapless::Vec<_, 2> = if self.contains(cut_out.start()) {
            if self.end() < cut_out.end() {
                heapless::Vec::from_slice(&[
                    *self.start()..=*cut_out.start() - 1,
                    *cut_out.end() + 1..=*self.end(),
                ])
                .unwrap()
            } else {
                if let Some(end_inclusive) = cut_out.start().checked_sub(1) {
                    heapless::Vec::from_slice(&[*self.start()..=end_inclusive]).unwrap()
                } else {
                    Default::default()
                }
            }
        } else if self.contains(cut_out.end()) {
            heapless::Vec::from_slice(&[*cut_out.end() + 1..=*self.end()]).unwrap()
        } else {
            heapless::Vec::from_slice(&[self]).unwrap()
        };
        result.into_iter()
    }
}
