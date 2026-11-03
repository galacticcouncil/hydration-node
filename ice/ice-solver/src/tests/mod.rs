mod fixtures_conformance;
mod flow_graph;
mod fmath;
mod passthrough;
mod ring_detection;
mod split;
mod tolerance;
mod v4_solver;

/// The mock markets price whole routes only, while hop `units` make the solver simulate
/// multi-hop routes hop by hop, so tests over them keep the flat per-route haircut.
pub(crate) fn flat_tolerance() -> crate::common::split::Tolerance {
	crate::common::split::Tolerance::parse("route=100,omnipool=0,stableswap=0,xyk=0,lbp=0,aave=0,hsm=0")
		.expect("valid tolerance")
}
