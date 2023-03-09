//! Order types
use std::cmp::Ordering;

/// Common API for limit orders
pub trait Order: Clone + Ord {
    type Opposite: Order;
    /// Whether the order's value is zero
    fn is_zero(&self) -> bool;
    /// Try fill this order with `other`
    fn try_fill(&mut self, other: &mut Self::Opposite) -> Option<(Fill, Fill)>;
}

#[derive(PartialEq, PartialOrd, Clone, Debug)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn opposite(&self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }
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

#[derive(PartialEq, Clone, Debug, Default)]
pub struct LimitOrder {
    // Note: field declaration order is important for sort implementation
    pub price: f32,
    pub nonce: u64,
    pub amount: u32,
    pub trader_id: u32,
}
#[derive(PartialEq, Clone, Debug, Default)]
pub struct BuyLimitOrder(LimitOrder);

impl From<LimitOrder> for BuyLimitOrder {
    fn from(f: LimitOrder) -> Self {
        Self(f)
    }
}

#[derive(PartialEq, Clone, Debug, Default)]
pub struct SellLimitOrder(LimitOrder);

impl From<LimitOrder> for SellLimitOrder {
    fn from(f: LimitOrder) -> Self {
        Self(f)
    }
}

impl Order for BuyLimitOrder {
    type Opposite = SellLimitOrder;
    #[inline(always)]
    fn is_zero(&self) -> bool {
        self.0.amount == 0
    }
    fn try_fill(&mut self, other: &mut Self::Opposite) -> Option<(Fill, Fill)> {
        if self.0.price >= other.0.price {
            self.0.try_fill(&mut other.0, OrderSide::Buy)
        } else {
            None
        }
    }
}

impl Order for SellLimitOrder {
    type Opposite = BuyLimitOrder;
    fn is_zero(&self) -> bool {
        self.0.amount == 0
    }
    fn try_fill(&mut self, other: &mut Self::Opposite) -> Option<(Fill, Fill)> {
        if self.0.price <= other.0.price {
            self.0.try_fill(&mut other.0, OrderSide::Sell)
        } else {
            None
        }
    }
}

impl LimitOrder {
    /// Try fill `self` with `other`, returns the resulting `Fill` events if any
    fn try_fill(&mut self, other: &mut LimitOrder, side: OrderSide) -> Option<(Fill, Fill)> {
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
                side.clone(),
                self.trader_id,
                other.trader_id,
            ),
            Fill::new(
                fill_amount,
                self.price,
                side.opposite(),
                other.trader_id,
                self.trader_id,
            ),
        ))
    }
}

impl PartialOrd for BuyLimitOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.0.price.total_cmp(&other.0.price) {
            Ordering::Equal => self.0.nonce.partial_cmp(&other.0.nonce),
            Ordering::Greater => Some(Ordering::Less),
            Ordering::Less => Some(Ordering::Greater),
        }
    }
}

impl Ord for BuyLimitOrder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other)
            .expect("only valid floats are given")
    }
}

impl PartialOrd for SellLimitOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.0.price.total_cmp(&other.0.price) {
            Ordering::Equal => self.0.nonce.partial_cmp(&other.0.nonce),
            order => Some(order),
        }
    }
}

impl Ord for SellLimitOrder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other)
            .expect("only valid floats are given")
    }
}

impl Eq for SellLimitOrder {}
impl Eq for BuyLimitOrder {}
