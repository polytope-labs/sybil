use crate::{NegativeImbalance, OnUnbalanced, Treasury};


pub struct HandleDustRemoval;
impl OnUnbalanced<NegativeImbalance> for HandleDustRemoval {
	fn on_nonzero_unbalanced(amount: NegativeImbalance) {
		Treasury::on_unbalanced(amount);
	}
}

