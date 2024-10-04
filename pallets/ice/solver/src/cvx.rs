use crate::traits::ICESolver;
use hydra_dx_math::ratio::Ratio;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider, RouterT};
use pallet_ice::types::{Balance, BoundedRoute, Intent, IntentId, ResolvedIntent, TradeInstruction};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::Saturating;
use sp_std::collections::btree_map::BTreeMap;
use crate::SolverSolution;

//use totsu::prelude::*;
//use totsu::*;

use clarabel::algebra::*;
use clarabel::solver::*;

pub struct CVXSolver<T, R, RP, PP>(sp_std::marker::PhantomData<(T, R, RP, PP)>);

impl<T: pallet_ice::Config, R, RP, PP> ICESolver<(IntentId, Intent<T::AccountId, <T as pallet_ice::Config>::AssetId>)>
for CVXSolver<T, R, RP, PP>
where
    <T as pallet_ice::Config>::AssetId: From<u32>,
    R: RouterT<
        T::RuntimeOrigin,
        <T as pallet_ice::Config>::AssetId,
        u128,
        hydradx_traits::router::Trade<<T as pallet_ice::Config>::AssetId>,
        hydradx_traits::router::AmountInAndOut<u128>,
    >,
    RP: RouteProvider<<T as pallet_ice::Config>::AssetId>,
    PP: PriceProvider<<T as pallet_ice::Config>::AssetId, Price = Ratio>,
{
    type Solution = SolverSolution<T::AssetId>;
    type Error = ();

    fn solve(
        intents: Vec<(IntentId, Intent<T::AccountId, <T as pallet_ice::Config>::AssetId>)>,
    ) -> Result<Self::Solution, Self::Error> {

        // QP Example

        // let P = CscMatrix::identity(2);    // For P = I
        // let P = CscMatrix::zeros((2,2));   // For P = 0

        // direct from sparse data
        let _P = CscMatrix::new(
            2,             // m
            2,             // n
            vec![0, 1, 2], // colptr
            vec![0, 1],    // rowval
            vec![6., 4.],  // nzval
        );

        // or an easier way for small problems...
        let P = CscMatrix::from(&[
            [6., 0.], //
            [0., 4.], //
        ]);

        let q = vec![-1., -4.];

        //direct from sparse data
        let _A = CscMatrix::new(
            5,                               // m
            2,                               // n
            vec![0, 3, 6],                   // colptr
            vec![0, 1, 3, 0, 2, 4],          // rowval
            vec![1., 1., -1., -2., 1., -1.], // nzval
        );

        // or an easier way for small problems...
        let A = CscMatrix::from(&[
            [1., -2.], // <-- LHS of equality constraint (lower bound)
            [1., 0.],  // <-- LHS of inequality constraint (upper bound)
            [0., 1.],  // <-- LHS of inequality constraint (upper bound)
            [-1., 0.], // <-- LHS of inequality constraint (lower bound)
            [0., -1.], // <-- LHS of inequality constraint (lower bound)
        ]);

        let b = vec![0., 1., 1., 1., 1.];

        let cones = [ZeroConeT(1), NonnegativeConeT(4)];

        let settings = DefaultSettings::default();

        let mut solver = DefaultSolver::new(&P, &q, &A, &b, &cones, settings);

        solver.solve();

        println!("Solution(x)     = {:?}", solver.solution.x);
        println!("Multipliers (z) = {:?}", solver.solution.z);
        println!("Slacks (s)      = {:?}", solver.solution.s);


        /*
        type La = FloatGeneric<f64>;
        type AMatBuild = MatBuild<La>;
        type AProbQP = ProbQP<La>;
        type ASolver = Solver<La>;

        let n = 2; // x0, x1
        let m = 1;
        let p = 0;

        // (1/2)(x - a)^2 + const
        let mut sym_p = AMatBuild::new(MatType::SymPack(n));
        sym_p[(0, 0)] = 1.;
        sym_p[(1, 1)] = 1.;

        let mut vec_q = AMatBuild::new(MatType::General(n, 1));
        vec_q[(0, 0)] = -(-1.); // -a0
        vec_q[(1, 0)] = -(-2.); // -a1

        // 1 - x0/b0 - x1/b1 <= 0
        let mut mat_g = AMatBuild::new(MatType::General(m, n));
        mat_g[(0, 0)] = -1. / 2.; // -1/b0
        mat_g[(0, 1)] = -1. / 3.; // -1/b1

        let mut vec_h = AMatBuild::new(MatType::General(m, 1));
        vec_h[(0, 0)] = -1.;

        let mat_a = AMatBuild::new(MatType::General(p, n));

        let vec_b = AMatBuild::new(MatType::General(p, 1));

        let s = ASolver::new().par(|p| {
            p.max_iter = Some(100_000);
        });
        let mut qp = AProbQP::new(sym_p, vec_q, mat_g, vec_h, mat_a, vec_b, s.par.eps_zero);
        let rslt = s.solve(qp.problem()).unwrap();
        
         */


        Err(())
    }
}
