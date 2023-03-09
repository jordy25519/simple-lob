#![feature(test)]
use std::cmp::Ordering;
use std::collections::VecDeque;

trait LOB {
    type Error;
    fn submit_order(
        &mut self,
        trader_id: u32,
        amount: u32,
        price: f32,
        side: OrderSide,
    ) -> Result<Vec<Fill>, Self::Error>;
}

#[derive(PartialEq, PartialOrd, Clone, Debug)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, PartialEq)]
pub struct Fill {
    pub side: OrderSide,
    pub amount: u32,
    pub price: f32,
    pub trader: u32,
    pub counter_party: u32,
}

impl Fill {
    pub fn new(amount: u32, price: f32, side: OrderSide, trader: u32, counter_party: u32) -> Self {
        Fill {
            amount,
            price,
            side,
            trader,
            counter_party,
        }
    }
}

/// Note: field declaration order is important for derived sort implementation
#[derive(PartialEq, PartialOrd, Clone, Debug)]
struct LimitOrder {
    side: OrderSide,
    nonce: u64,
    amount: u32,
    price: f32,
    trader_id: u32,
}

impl LimitOrder {
    /// Try fill `self` with `other`, returns the resulting `Fill` events if any
    fn try_fill(&mut self, other: &mut LimitOrder) -> Option<(Fill, Fill)> {
        if (self.side == OrderSide::Buy && self.price < other.price)
            || (self.side == OrderSide::Sell && self.price > other.price)
        {
            return None;
        }
        let fill_amount = match self.amount.cmp(&other.amount) {
            Ordering::Equal | Ordering::Less => {
                let amount = self.amount;
                other.amount -= amount;
                self.amount = 0;
                amount
            }
            Ordering::Greater => {
                let amount = other.amount;
                self.amount -= amount;
                other.amount = 0;
                amount
            }
        };

        Some((
            Fill::new(
                fill_amount,
                self.price,
                self.side.clone(),
                self.trader_id,
                other.trader_id,
            ),
            Fill::new(
                fill_amount,
                self.price,
                other.side.clone(),
                other.trader_id,
                self.trader_id,
            ),
        ))
    }
}

impl Ord for LimitOrder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other)
            .expect("only valid floats are given")
    }
}

impl Eq for LimitOrder {}

#[derive(Default, Debug)]
struct OrderBook(VecDeque<LimitOrder>);

impl OrderBook {
    pub fn front(&self) -> Option<&LimitOrder> {
        self.0.front()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Insert an order into the book at the correct location
    pub fn insert_order(&mut self, order: &LimitOrder) -> Result<(), ()> {
        if let Err(idx) = self.0.binary_search(order) {
            self.0.insert(idx, order.clone());
            Ok(())
        } else {
            Err(())
        }
    }
    /// Submit an order to the book
    /// Returning fills and remaining unfilled order if any
    pub fn submit_order<'a>(
        &mut self,
        order: &'a mut LimitOrder,
    ) -> (Vec<Fill>, Option<&'a LimitOrder>) {
        // try add the order to the book absorbing any resting liquidity
        let mut fills = Vec::<Fill>::default();
        let mut remove_count = 0;
        for resting_order in self.0.iter_mut() {
            if let Some((fill_0, fill_1)) = resting_order.try_fill(order) {
                fills.push(fill_0);
                fills.push(fill_1);
                if resting_order.amount == 0 {
                    remove_count += 1;
                }
            } else {
                break;
            }
            if order.amount == 0 {
                break;
            }
        }

        // Remove filled orders from the book
        if remove_count > 0 {
            let _ = self.0.drain(0..remove_count);
        }

        if order.amount > 0 {
            (fills, Some(order))
        } else {
            (fills, None)
        }
    }
}

#[derive(Default)]
struct Market {
    /// Order nonce
    nonce: u64,
    buys: OrderBook,
    sells: OrderBook,
}

impl LOB for Market {
    type Error = ();
    fn submit_order(
        &mut self,
        trader_id: u32,
        amount: u32,
        price: f32,
        side: OrderSide,
    ) -> Result<Vec<Fill>, Self::Error> {
        let mut order = LimitOrder {
            price,
            amount,
            trader_id,
            side: side.clone(),
            nonce: self.nonce,
        };

        let fills = match side {
            OrderSide::Buy => {
                let (fills, unfilled) = self.sells.submit_order(&mut order);
                if let Some(unfilled) = unfilled {
                    self.buys
                        .insert_order(unfilled)
                        .expect("orderbook has capacity");
                }
                fills
            }
            OrderSide::Sell => {
                let (fills, unfilled) = self.buys.submit_order(&mut order);
                if let Some(unfilled) = unfilled {
                    self.sells
                        .insert_order(unfilled)
                        .expect("orderbook has capacity");
                }
                fills
            }
        };

        self.nonce += 1;
        Ok(fills)
    }
}

#[cfg(test)]
mod tests {
    use crate::{LimitOrder, Market, OrderSide, LOB};
    use std::{hint::black_box, time::Duration};

    extern crate test;
    use test::Bencher;

    #[test]
    fn add_resting_buys() {
        let mut lob = Market::default();

        for i in 1_u32..=5 {
            assert_eq!(
                lob.submit_order(i, 100 * i, i as f32 * 1.0_f32, OrderSide::Buy),
                Ok(vec![]),
            );
        }

        let fills = lob.submit_order(5_u32, 150, 1_f32 * 1.0_f32, OrderSide::Sell);
        println!("{:?}", fills);
        let fills = lob.submit_order(5_u32, 1_350, 1_f32 * 1.0_f32, OrderSide::Sell);
        println!("{:?}", fills);

        println!("{:?}", lob.buys);
        println!("{:?}", lob.sells);
    }

    #[test]
    fn add_resting_sells() {
        let mut lob = Market::default();

        for i in 1_u32..=5 {
            assert_eq!(
                lob.submit_order(i, 100 * i, i as f32 * 1.0_f32, OrderSide::Sell),
                Ok(vec![]),
            );
        }

        let charlie_id = 5_u32;

        let fills = lob.submit_order(charlie_id, 150, 5_f32 * 1.0_f32, OrderSide::Buy);
        println!("{:?}", fills);
        let fills = lob.submit_order(charlie_id, 1_450, 5_f32 * 1.0_f32, OrderSide::Buy);
        println!("{:?}", fills);

        assert!(lob.sells.is_empty());
        assert_eq!(
            lob.buys.front(),
            Some(&LimitOrder {
                trader_id: charlie_id,
                price: 5_f32,
                amount: 100,
                nonce: 6,
                side: OrderSide::Buy,
            })
        );
    }

    #[bench]
    fn bench_market_orders(b: &mut Bencher) {
        b.iter(|| black_box(bench_1()));
    }

    fn bench_1() {
        let mut lob = Market::default();
        for i in 1..=100_000_u32 {
            black_box(assert!(lob
                .submit_order(i, 1, 1.0_f32, OrderSide::Sell)
                .is_ok()));
        }
        for i in 1..=100_000_u32 {
            black_box(assert!(lob
                .submit_order(i, 1, 1.0_f32, OrderSide::Buy)
                .is_ok()));
        }
    }

    #[test]
    fn bench_1_t() {
        use std::time::Instant;
        let mut diffs = vec![];
        for _ in 0..100 {
            let s_0 = Instant::now();
            black_box(bench_1());
            let s_1 = Instant::now();
            diffs.push(s_1 - s_0);
        }
        let s_m: Duration = diffs.iter().sum();
        println!("{:?}", s_m / diffs.len() as u32);
        assert!(false);
    }

    #[bench]
    fn bench_random_orders(b: &mut Bencher) {
        b.iter(|| black_box(bench_2()));
    }

    fn bench_2() {
        use rand::Rng;

        let mut lob = Market::default();
        for i in 1_u32..=100_000 {
            let price_r = rand::thread_rng().gen_range(1..10_000);
            black_box(assert!(lob
                .submit_order(i, 1, price_r as f32, OrderSide::Sell)
                .is_ok()));
        }

        for i in 1_u32..=100_000 {
            let price_r = rand::thread_rng().gen_range(1..10_000);
            black_box(assert!(lob
                .submit_order(i, 1, price_r as f32, OrderSide::Buy)
                .is_ok()));
        }
    }

    #[test]
    fn bench_2_t() {
        use std::time::Instant;
        let mut diffs = vec![];
        for _ in 0..100 {
            let s_0 = Instant::now();
            black_box(bench_2());
            let s_1 = Instant::now();
            diffs.push(s_1 - s_0);
        }
        let s_m: Duration = diffs.iter().sum();
        println!("{:?}", s_m / diffs.len() as u32);
        assert!(false);
    }
}
