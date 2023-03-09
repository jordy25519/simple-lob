//! Simple limit order book

use std::{cmp::Ordering, collections::VecDeque};

/// Provides a limit order book API
pub trait LOB {
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

// An event denoting a matched order
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
#[derive(PartialEq, Clone, Debug)]
struct LimitOrder {
    price: f32,
    nonce: u64,
    // TODO: this could be handled by the type system or inferred
    // TODO: the sorting is reversed for each side of the book
    side: OrderSide,
    amount: u32,
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

impl PartialOrd for LimitOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.price.total_cmp(&other.price) {
            Ordering::Equal => self.nonce.partial_cmp(&other.nonce),
            Ordering::Greater => match self.side {
                OrderSide::Buy => Some(Ordering::Less),
                OrderSide::Sell => Some(Ordering::Greater),
            },
            Ordering::Less => match self.side {
                OrderSide::Buy => Some(Ordering::Greater),
                OrderSide::Sell => Some(Ordering::Less),
            },
        }
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
pub struct Market {
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
pub mod tests {
    use crate::{Fill, LimitOrder, Market, OrderSide, LOB};

    #[test]
    fn orders_sort_by_price_then_nonce() {
        let mut orders = vec![
            LimitOrder {
                trader_id: 1,
                nonce: 2,
                price: 2.0,
                amount: 1,
                side: OrderSide::Buy,
            },
            LimitOrder {
                trader_id: 1,
                nonce: 1,
                price: 2.0,
                amount: 1,
                side: OrderSide::Buy,
            },
            LimitOrder {
                trader_id: 1,
                nonce: 3,
                price: 1.0,
                amount: 1,
                side: OrderSide::Buy,
            },
        ];
        orders.sort();

        assert_eq!(
            orders.as_slice(),
            &[
                LimitOrder {
                    trader_id: 1,
                    nonce: 1,
                    price: 2.0,
                    amount: 1,
                    side: OrderSide::Buy,
                },
                LimitOrder {
                    trader_id: 1,
                    nonce: 2,
                    price: 2.0,
                    amount: 1,
                    side: OrderSide::Buy,
                },
                LimitOrder {
                    trader_id: 1,
                    nonce: 3,
                    price: 1.0,
                    amount: 1,
                    side: OrderSide::Buy,
                },
            ]
        );

        orders = vec![
            LimitOrder {
                trader_id: 1,
                nonce: 2,
                price: 2.0,
                amount: 1,
                side: OrderSide::Sell,
            },
            LimitOrder {
                trader_id: 1,
                nonce: 1,
                price: 2.0,
                amount: 1,
                side: OrderSide::Sell,
            },
            LimitOrder {
                trader_id: 1,
                nonce: 3,
                price: 1.0,
                amount: 1,
                side: OrderSide::Sell,
            },
        ];
        orders.sort();
        assert_eq!(
            orders.as_slice(),
            &[
                LimitOrder {
                    trader_id: 1,
                    nonce: 3,
                    price: 1.0,
                    amount: 1,
                    side: OrderSide::Sell,
                },
                LimitOrder {
                    trader_id: 1,
                    nonce: 1,
                    price: 2.0,
                    amount: 1,
                    side: OrderSide::Sell,
                },
                LimitOrder {
                    trader_id: 1,
                    nonce: 2,
                    price: 2.0,
                    amount: 1,
                    side: OrderSide::Sell,
                },
            ]
        );
    }

    #[test]
    fn add_resting_buys() {
        let mut lob = Market::default();

        for i in 1_u32..=5 {
            assert_eq!(
                lob.submit_order(i, 100 * i, i as f32 * 1.0_f32, OrderSide::Buy),
                Ok(vec![]),
            );
        }

        let seller_id = 6_u32;
        let fills = lob
            .submit_order(seller_id, 550, 1.0_f32, OrderSide::Sell)
            .unwrap();
        assert_eq!(
            fills.as_slice(),
            &[
                Fill::new(500, 5.0, OrderSide::Buy, 5, seller_id,),
                Fill::new(500, 5.0, OrderSide::Sell, seller_id, 5,),
                Fill::new(50, 4.0, OrderSide::Buy, 4, seller_id,),
                Fill::new(50, 4.0, OrderSide::Sell, seller_id, 4,),
            ]
        );
        let _fills = lob.submit_order(seller_id, 1050, 1_f32 * 1.0_f32, OrderSide::Sell);

        assert!(lob.buys.is_empty());
        assert_eq!(
            lob.sells.front(),
            Some(&LimitOrder {
                trader_id: seller_id,
                price: 1_f32,
                amount: 100,
                nonce: 6,
                side: OrderSide::Sell,
            })
        );
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
        let buyer_id = 5_u32;

        let fills = lob
            .submit_order(buyer_id, 150, 5.0, OrderSide::Buy)
            .unwrap();
        assert_eq!(
            fills.as_slice(),
            &[
                Fill::new(100, 1.0, OrderSide::Sell, 1, buyer_id,),
                Fill::new(100, 1.0, OrderSide::Buy, buyer_id, 1),
                Fill::new(50, 2.0, OrderSide::Sell, 2, buyer_id,),
                Fill::new(50, 2.0, OrderSide::Buy, buyer_id, 2),
            ]
        );

        let _fills = lob.submit_order(buyer_id, 1_450, 5.0, OrderSide::Buy);

        assert!(lob.sells.is_empty());
        assert_eq!(
            lob.buys.front(),
            Some(&LimitOrder {
                trader_id: buyer_id,
                price: 5_f32,
                amount: 100,
                nonce: 6,
                side: OrderSide::Buy,
            })
        );
    }

    #[test]
    fn unfilled_buy() {
        let mut lob = Market::default();

        assert_eq!(
            lob.submit_order(1, 100, 5.0_f32, OrderSide::Sell),
            Ok(vec![]),
        );

        let fills = lob.submit_order(2, 100, 4.0, OrderSide::Buy).unwrap();
        assert!(fills.is_empty());

        assert_eq!(
            lob.buys.front(),
            Some(&LimitOrder {
                trader_id: 2,
                price: 4.0,
                amount: 100,
                nonce: 1,
                side: OrderSide::Buy,
            })
        );
    }

    #[test]
    fn unfilled_sell() {
        let mut lob = Market::default();

        assert_eq!(
            lob.submit_order(1, 100, 4.0_f32, OrderSide::Buy),
            Ok(vec![]),
        );

        let fills = lob.submit_order(2, 100, 5.0, OrderSide::Sell).unwrap();
        assert!(fills.is_empty());

        assert_eq!(
            lob.sells.front(),
            Some(&LimitOrder {
                trader_id: 2,
                price: 5.0,
                amount: 100,
                nonce: 1,
                side: OrderSide::Sell,
            })
        );
    }
}
