//! Simple limit order book

use std::collections::VecDeque;

mod order;
pub use order::{BuyLimitOrder, Fill, LimitOrder, Order, OrderSide, SellLimitOrder};

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

#[derive(Default, Debug)]
struct OrderBook<T: Order>(VecDeque<T>);

impl<T: Order> OrderBook<T> {
    pub fn front(&self) -> Option<&T> {
        self.0.front()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Insert an order into the book at the correct location
    pub fn insert_order(&mut self, order: &T) -> Result<(), ()> {
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
        order: &'a mut T::Opposite,
    ) -> (Vec<Fill>, Option<&'a T::Opposite>) {
        // try add the order to the book absorbing any resting liquidity
        let mut fills = Vec::<Fill>::default();
        let mut remove_count = 0;
        for resting_order in self.0.iter_mut() {
            if let Some((fill_0, fill_1)) = resting_order.try_fill(order) {
                fills.push(fill_0);
                fills.push(fill_1);
                if resting_order.is_zero() {
                    remove_count += 1;
                }
            } else {
                break;
            }
            if order.is_zero() {
                break;
            }
        }

        // Remove filled orders from the book
        if remove_count > 0 {
            let _ = self.0.drain(0..remove_count);
        }

        if order.is_zero() {
            (fills, None)
        } else {
            (fills, Some(order))
        }
    }
}

#[derive(Default)]
pub struct Market {
    /// Order nonce
    nonce: u64,
    buys: OrderBook<BuyLimitOrder>,
    sells: OrderBook<SellLimitOrder>,
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
        if amount == 0 {
            return Ok(vec![]);
        }

        let order = LimitOrder {
            price,
            amount,
            trader_id,
            nonce: self.nonce,
        };

        let fills = match side {
            OrderSide::Buy => {
                let mut order = order.into();
                let (fills, unfilled) = self.sells.submit_order(&mut order);
                if let Some(unfilled) = unfilled {
                    self.buys
                        .insert_order(unfilled)
                        .expect("orderbook has capacity");
                }
                fills
            }
            OrderSide::Sell => {
                let mut order = order.into();
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
    use crate::{BuyLimitOrder, Fill, LimitOrder, Market, OrderSide, SellLimitOrder, LOB};

    #[test]
    fn orders_sort_by_price_then_nonce() {
        let mut orders: Vec<BuyLimitOrder> = vec![
            LimitOrder {
                trader_id: 1,
                nonce: 2,
                price: 2.0,
                amount: 1,
            }
            .into(),
            LimitOrder {
                trader_id: 1,
                nonce: 1,
                price: 2.0,
                amount: 1,
            }
            .into(),
            LimitOrder {
                trader_id: 1,
                nonce: 3,
                price: 1.0,
                amount: 1,
            }
            .into(),
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
                }
                .into(),
                LimitOrder {
                    trader_id: 1,
                    nonce: 2,
                    price: 2.0,
                    amount: 1,
                }
                .into(),
                LimitOrder {
                    trader_id: 1,
                    nonce: 3,
                    price: 1.0,
                    amount: 1,
                }
                .into(),
            ]
        );

        let mut orders: Vec<SellLimitOrder> = vec![
            LimitOrder {
                trader_id: 1,
                nonce: 2,
                price: 2.0,
                amount: 1,
            }
            .into(),
            LimitOrder {
                trader_id: 1,
                nonce: 1,
                price: 2.0,
                amount: 1,
            }
            .into(),
            LimitOrder {
                trader_id: 1,
                nonce: 3,
                price: 1.0,
                amount: 1,
            }
            .into(),
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
                }
                .into(),
                LimitOrder {
                    trader_id: 1,
                    nonce: 1,
                    price: 2.0,
                    amount: 1,
                }
                .into(),
                LimitOrder {
                    trader_id: 1,
                    nonce: 2,
                    price: 2.0,
                    amount: 1,
                }
                .into(),
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
            Some(
                &LimitOrder {
                    trader_id: seller_id,
                    price: 1_f32,
                    amount: 100,
                    nonce: 6,
                }
                .into()
            )
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
            Some(
                &LimitOrder {
                    trader_id: buyer_id,
                    price: 5_f32,
                    amount: 100,
                    nonce: 6,
                }
                .into()
            )
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
            Some(
                &LimitOrder {
                    trader_id: 2,
                    price: 4.0,
                    amount: 100,
                    nonce: 1,
                }
                .into()
            )
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
            Some(
                &LimitOrder {
                    trader_id: 2,
                    price: 5.0,
                    amount: 100,
                    nonce: 1,
                }
                .into()
            )
        );
    }
}
