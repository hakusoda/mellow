use serde::Deserialize;

use crate::hakumi::visual_scripting::{ Variable, VariableKind };

#[derive(Clone)]
pub struct CampaignModel {
	pub tiers: Vec<Tier>
}

impl From<CampaignModel> for Variable {
	fn from(value: CampaignModel) -> Self {
		Variable::create_map([
			("tiers", VariableKind::List(value.tiers.into_iter().map(|x| Variable::create_map([
				("patron_count", x.patron_count.into())
			], None)).collect()).into())
		], None)
	}
}

#[derive(Clone)]
pub struct Tier {
	pub patron_count: u64
}

#[derive(Deserialize)]
pub struct GetCampaign {
	pub included: Option<Vec<IncludedItem>>
}

#[derive(Deserialize)]
pub struct IncludedItem {
	pub attributes: IncludedItemAttributes
}

#[derive(Deserialize)]
pub struct IncludedItemAttributes {
	pub patron_count: u64
}