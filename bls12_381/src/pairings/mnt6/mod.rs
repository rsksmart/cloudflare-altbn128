use crate::field::SizedPrimeField;
use crate::fp::Fp;
use crate::representation::ElementRepr;
use crate::traits::{FieldElement, MsbBitIterator, ZeroAndOne};
use crate::weierstrass::{CurveParameters};
use crate::weierstrass::curve::{WeierstrassCurve, CurvePoint};
use crate::extension_towers::fp3::{Fp3, Extension3};
use crate::extension_towers::fp6_as_2_over_3::{Fp6, Extension2Over3};
use crate::pairings::PairingEngine;
use crate::pairings::{calculate_bits, calculate_hamming_weight, calculate_naf_hamming_weight, into_ternary_wnaf};
use crate::weierstrass::Group;

#[derive(Clone)]
pub struct MNT6InstanceParams<
    'a, 
        FE: ElementRepr, 
        F: SizedPrimeField<Repr = FE>, 
        CB: CurveParameters<BaseFieldElement = Fp<'a, FE, F>>,
        CTW: CurveParameters<BaseFieldElement = Fp3<'a, FE, F>>
    > {
    pub x: &'a [u64],
    pub x_is_negative: bool,
    pub exp_w0: &'a [u64],
    pub exp_w1: &'a [u64],
    pub exp_w0_is_negative: bool,
    pub base_field: &'a F,
    pub curve: &'a WeierstrassCurve<'a, CB>,
    pub curve_twist: &'a WeierstrassCurve<'a, CTW>,
    pub twist: Fp3<'a, FE, F>,
    pub fp3_extension: &'a Extension3<'a, FE, F>,
    pub fp6_extension: &'a Extension2Over3<'a, FE, F>,    
    pub force_no_naf: bool
}

#[derive(Clone)]
pub struct MNT6Instance<
        'a, 
        FE: ElementRepr, 
        F: SizedPrimeField<Repr = FE>, 
        CB: CurveParameters<BaseFieldElement = Fp<'a, FE, F>>,
        CTW: CurveParameters<BaseFieldElement = Fp3<'a, FE, F>>
    > {
    pub x: &'a [u64],
    pub x_is_negative: bool,
    pub exp_w0: &'a [u64],
    pub exp_w1: &'a [u64],
    pub exp_w0_is_negative: bool,
    pub base_field: &'a F,
    pub curve: &'a WeierstrassCurve<'a, CB>,
    pub curve_twist: &'a WeierstrassCurve<'a, CTW>,
    pub twist: Fp3<'a, FE, F>,
    pub fp3_extension: &'a Extension3<'a, FE, F>,
    pub fp6_extension: &'a Extension2Over3<'a, FE, F>,
    pub prefer_naf: bool,
    pub x_naf: Vec<i8>
}


impl<
    'a, 
        FE: ElementRepr, 
        F: SizedPrimeField<Repr = FE>, 
        CB: CurveParameters<BaseFieldElement = Fp<'a, FE, F>>,
        CTW: CurveParameters<BaseFieldElement = Fp3<'a, FE, F>>
    > MNT6Instance<'a, FE, F, CB, CTW> 
{
    pub fn from_params(params: MNT6InstanceParams::<'a, FE, F, CB, CTW>) -> Self {
        let (prefer_naf, naf) = if params.force_no_naf {
            (false, vec![])
        } else {
            let naf_vec = into_ternary_wnaf(&params.x);
            let original_bits = calculate_bits(&params.x);
            let original_hamming = calculate_hamming_weight(&params.x);
            let naf_hamming = calculate_naf_hamming_weight(&naf_vec);
            let naf_length = naf_vec.len() as u32;

            let naf_is_beneficial = if naf_length + naf_hamming < original_bits + original_hamming {
                true
            } else {
                false
            };

            if naf_is_beneficial {
                (true, naf_vec)
            } else {
                (false, vec![])
            }
        };

        Self {
            x: params.x,
            x_is_negative: params.x_is_negative,
            exp_w0: params.exp_w0,
            exp_w1: params.exp_w1,
            exp_w0_is_negative: params.exp_w0_is_negative,
            base_field: params.base_field,
            curve: params.curve,
            curve_twist: params.curve_twist,
            twist: params.twist,
            fp3_extension: params.fp3_extension,
            fp6_extension: params.fp6_extension,
            prefer_naf: prefer_naf,
            x_naf: naf
        }
    }
}

struct PrecomputedG1<'a, FE: ElementRepr, F: SizedPrimeField<Repr = FE>> {
    pub x: Fp<'a, FE, F>,
    pub y: Fp<'a, FE, F>,
    pub x_by_twist: Fp3<'a, FE, F>,
    pub y_by_twist: Fp3<'a, FE, F>,
}

struct PrecomputedG2<'a, FE: ElementRepr, F: SizedPrimeField<Repr = FE>> {
    pub x: Fp3<'a, FE, F>,
    pub y: Fp3<'a, FE, F>,
    pub x_over_twist: Fp3<'a, FE, F>,
    pub y_over_twist: Fp3<'a, FE, F>,
    pub double_coefficients: Vec<AteDoubleCoefficients<'a, FE, F>>,
    pub addition_coefficients: Vec<AteAdditionCoefficients<'a, FE, F>>,
}

struct AteDoubleCoefficients<'a, FE: ElementRepr, F: SizedPrimeField<Repr = FE>> {
    pub c_h:  Fp3<'a, FE, F>,
    pub c_4c: Fp3<'a, FE, F>,
    pub c_j:  Fp3<'a, FE, F>,
    pub c_l:  Fp3<'a, FE, F>,
}

struct AteAdditionCoefficients<'a, FE: ElementRepr, F: SizedPrimeField<Repr = FE>> {
    pub c_l1: Fp3<'a, FE, F>,
    pub c_rz: Fp3<'a, FE, F>,
}

struct ExtendedCoordinates<'a, FE: ElementRepr, F: SizedPrimeField<Repr = FE>> {
    pub x: Fp3<'a, FE, F>,
    pub y: Fp3<'a, FE, F>,
    pub z: Fp3<'a, FE, F>,
    pub t: Fp3<'a, FE, F>,
}

impl<
    'a, 
        FE: ElementRepr, 
        F: SizedPrimeField<Repr = FE>, 
        CB: CurveParameters<BaseFieldElement = Fp<'a, FE, F>>,
        CTW: CurveParameters<BaseFieldElement = Fp3<'a, FE, F>>
    > MNT6Instance<'a, FE, F, CB, CTW> {
    fn miller_loop<'b, I>(&self, i: I) -> Result<Fp6<'a, FE, F>, ()>
    where 'a: 'b,
        I: IntoIterator<
            Item = &'b (&'b CurvePoint<'a, CB>, 
                &'b CurvePoint<'a, CTW>)
        >
    {
        let mut f = Fp6::one(self.fp6_extension);
        for (p, q) in i.into_iter() {
            f.mul_assign(&self.ate_pairing_loop(p, q)?);
        }

        Ok(f)
    }

    fn precompute_g1(&self, g1_point: &CurvePoint<'a, CB>) -> PrecomputedG1<'a, FE, F> {
        // not asserting normalization, it will be asserted in the loop
        let mut x_twist = self.twist.clone();
        x_twist.mul_by_fp(&g1_point.x);

        let mut y_twist = self.twist.clone();
        y_twist.mul_by_fp(&g1_point.y);

        PrecomputedG1 {
            x: g1_point.x.clone(),
            y: g1_point.y.clone(),
            x_by_twist: x_twist,
            y_by_twist: y_twist,
        }
    }

    fn doubling_step(&self, r: &mut ExtendedCoordinates<'a, FE, F>) -> AteDoubleCoefficients<'a, FE, F> {
        let mut a = r.t.clone();
        a.square();
        let mut b = r.x.clone();
        b.square();
        let mut c = r.y.clone();
        c.square();
        let mut d = c.clone();
        d.square();

        let mut e = r.x.clone();
        e.add_assign(&c);
        e.square();
        e.sub_assign(&b);
        e.sub_assign(&d);

        let mut f = self.curve_twist.a.clone();
        f.mul_assign(&a);
        f.add_assign(&b);
        f.add_assign(&b);
        f.add_assign(&b);

        let mut g = f.clone();
        g.square();

        let mut d_eight = d.clone();
        d_eight.double();
        d_eight.double();
        d_eight.double();

        let mut t0 = e.clone();
        t0.double();
        t0.double();

        let mut x = g.clone();
        x.sub_assign(&t0);

        let mut y = e.clone();
        y.double();
        y.sub_assign(&x);
        y.mul_assign(&f);
        y.sub_assign(&d_eight);

        let mut t0 = r.z.clone();
        t0.square();

        let mut z = r.y.clone();
        z.add_assign(&r.z);
        z.square();
        z.sub_assign(&c);
        z.sub_assign(&t0);

        let mut t = z.clone();
        t.square();

        let mut c_h = z.clone();
        c_h.add_assign(&r.t);
        c_h.square();
        c_h.sub_assign(&t);
        c_h.sub_assign(&a);

        let mut c_4c = c.clone();
        c_4c.double();
        c_4c.double();

        let mut c_j = f.clone();
        c_j.add_assign(&r.t);
        c_j.square();
        c_j.sub_assign(&g);
        c_j.sub_assign(&a);

        let mut c_l = f.clone();
        c_l.add_assign(&r.x);
        c_l.square();
        c_l.sub_assign(&g);
        c_l.sub_assign(&b);

        let coeff = AteDoubleCoefficients { c_h, c_4c, c_j, c_l};

        r.x = x;
        r.y = y;
        r.z = z;
        r.t = t;

        coeff
    }

    fn addition_step(&self,         
        x: &Fp3<'a, FE, F>,
        y: &Fp3<'a, FE, F>,
        r: &mut ExtendedCoordinates<'a, FE, F>) 
    -> AteAdditionCoefficients<'a, FE, F> {
        let mut a = y.clone();
        a.square();
        let mut b = r.t.clone();
        b.mul_assign(&x);

        let mut d = r.z.clone();
        d.add_assign(&y);
        d.square();
        d.sub_assign(&a);
        d.sub_assign(&r.t);
        d.mul_assign(&r.t);

        let mut h = b.clone();
        h.sub_assign(&r.x);

        let mut i = h.clone();
        i.square();

        let mut e = i.clone();
        e.double();
        e.double();

        let mut j = h.clone();
        j.mul_assign(&e);

        let mut v = r.x.clone();
        v.mul_assign(&e);

        let mut l1 = d.clone();
        l1.sub_assign(&r.y);
        l1.sub_assign(&r.y);

        let mut x = l1.clone();
        x.square();
        x.sub_assign(&j);
        x.sub_assign(&v);
        x.sub_assign(&v);

        let mut t0 = r.y.clone();
        t0.double();
        t0.mul_assign(&j);

        let mut y = v.clone();
        y.sub_assign(&x);
        y.mul_assign(&l1);
        y.sub_assign(&t0);

        let mut z = r.z.clone();
        z.add_assign(&h);
        z.square();
        z.sub_assign(&r.t);
        z.sub_assign(&i);

        let mut t = z.clone();
        t.square();

        let coeff = AteAdditionCoefficients { c_l1: l1, c_rz: z.clone() };

        r.x = x;
        r.y = y;
        r.z = z;
        r.t = t;

        coeff
    }


    fn precompute_g2(&self, g2_point: &CurvePoint<'a, CTW>, twist_inv: &Fp3<'a, FE, F>) -> Result<PrecomputedG2<'a, FE, F>, ()> {
        // not asserting normalization, it will be asserted in the loop
        // precompute addition and doubling coefficients
        let mut x_over_twist = g2_point.x.clone();
        x_over_twist.mul_assign(&twist_inv);

        let mut y_over_twist = g2_point.y.clone();
        y_over_twist.mul_assign(&twist_inv);

        let mut g2_p = PrecomputedG2 {
            x: g2_point.x.clone(),
            y: g2_point.y.clone(),
            x_over_twist: x_over_twist,
            y_over_twist: y_over_twist,
            double_coefficients: vec![],
            addition_coefficients: vec![],
        };

        let mut r = ExtendedCoordinates {
            x: g2_point.x.clone(),
            y: g2_point.y.clone(),
            z: Fp3::one(&self.fp3_extension),
            t: Fp3::one(&self.fp3_extension),
        };

        for bit in MsbBitIterator::new(&self.x).skip(1) {
            let coeff = self.doubling_step(&mut r);
            g2_p.double_coefficients.push(coeff);

            if bit {
                let coeff = self.addition_step(&g2_point.x, &g2_point.y, &mut r);
                g2_p.addition_coefficients.push(coeff);
            }
        }

        if self.x_is_negative {
            let rz_inv = r.z.inverse().ok_or(())?;
            let mut rz2_inv = rz_inv.clone();
            rz2_inv.square();
            let mut rz3_inv = rz_inv.clone();
            rz3_inv.mul_assign(&rz2_inv);

            let mut minus_r_affine_x = rz2_inv;
            minus_r_affine_x.mul_assign(&r.x);
            let mut minus_r_affine_y = rz3_inv;
            minus_r_affine_y.mul_assign(&r.y);
            minus_r_affine_y.negate();

            let coeff = self.addition_step(
                &minus_r_affine_x,
                &minus_r_affine_y,
                &mut r,
            );

            g2_p.addition_coefficients.push(coeff);
        }

        Ok(g2_p)
    }

    fn ate_pairing_loop(
        &self, 
        point: &CurvePoint<'a, CB>, 
        twist_point: &CurvePoint<'a, CTW> 
    ) -> Result<Fp6<'a, FE, F>, ()> {
        debug_assert!(point.is_normalized());
        debug_assert!(twist_point.is_normalized());

        let twist_inv = self.twist.inverse().ok_or(())?;

        let p = self.precompute_g1(&point);
        let q = self.precompute_g2(&twist_point, &twist_inv)?;
        let mut l1_coeff = Fp3::zero(&self.fp3_extension);
        l1_coeff.c0 = p.x.clone();
        l1_coeff.sub_assign(&q.x_over_twist);

        let mut f = Fp6::one(self.fp6_extension);

        let mut dbl_idx: usize = 0;
        let mut add_idx: usize = 0;

        // The for loop is executed for all bits (EXCEPT the MSB itself) of
        for bit in MsbBitIterator::new(&self.x).skip(1) {

            let dc = &q.double_coefficients[dbl_idx];
            dbl_idx += 1;

            let mut g_rr_at_p = Fp6::zero(&self.fp6_extension);

            let mut t0 = dc.c_j.clone();
            t0.mul_assign(&p.x_by_twist);
            t0.negate();
            t0.add_assign(&dc.c_l);
            t0.sub_assign(&dc.c_4c);

            let mut t1 = dc.c_h.clone();
            t1.mul_assign(&p.y_by_twist);

            g_rr_at_p.c0 = t0;
            g_rr_at_p.c1 = t1;

            f.square();
            f.mul_assign(&g_rr_at_p);

            if bit {
                let ac = &q.addition_coefficients[add_idx];
                add_idx += 1;

                let mut g_rq_at_p = Fp6::zero(&self.fp6_extension);

                let mut t0 = ac.c_rz.clone();
                t0.mul_assign(&p.y_by_twist);

                let mut t = l1_coeff.clone();
                t.mul_assign(&ac.c_l1);

                let mut t1 = q.y_over_twist.clone();
                t1.mul_assign(&ac.c_rz);
                t1.add_assign(&t);
                t1.negate();

                g_rq_at_p.c0 = t0;
                g_rq_at_p.c1 = t1;

                f.mul_assign(&g_rq_at_p);
            }
        }

        if self.x_is_negative {
            let ac = &q.addition_coefficients[add_idx];

            let mut g_rnegr_at_p = Fp6::zero(&self.fp6_extension);

            let mut t0 = ac.c_rz.clone();
            t0.mul_assign(&p.y_by_twist);

            let mut t = l1_coeff.clone();
            t.mul_assign(&ac.c_l1);

            let mut t1 = q.y_over_twist.clone();
            t1.mul_assign(&ac.c_rz);
            t1.add_assign(&t);
            t1.negate();

            g_rnegr_at_p.c0 = t0;
            g_rnegr_at_p.c1 = t1;

            f.mul_assign(&g_rnegr_at_p);
            f = f.inverse().ok_or(())?;
        }

        Ok(f)
    }

    fn final_exponentiation(&self, f: &Fp6<'a, FE, F>) -> Option<Fp6<'a, FE, F>> {
        let value_inv = f.inverse();
        if value_inv.is_none() {
            return None;
        }
        let value_inv = value_inv.expect("is some");
        let value_to_first_chunk = self.final_exponentiation_part_one(f, &value_inv);
        let value_inv_to_first_chunk = self.final_exponentiation_part_one(&value_inv, f);
        
        Some(self.final_exponentiation_part_two(&value_to_first_chunk, &value_inv_to_first_chunk))
    }

    fn final_exponentiation_part_one(&self, elt: &Fp6<'a, FE, F>, elt_inv: &Fp6<'a, FE, F>) -> Fp6<'a, FE, F> {
        // (q^3-1)*(q+1)

        // elt_q3 = elt^(q^3)
        let mut elt_q3 = elt.clone();
        elt_q3.frobenius_map(3);
        // elt_q3_over_elt = elt^(q^3-1)
        let mut elt_q3_over_elt = elt_q3;
        elt_q3_over_elt.mul_assign(&elt_inv);
        // alpha = elt^((q^3-1) * q)
        let mut alpha = elt_q3_over_elt.clone();
        alpha.frobenius_map(1);
        // beta = elt^((q^3-1)*(q+1)
        alpha.mul_assign(&elt_q3_over_elt);

        alpha
    }

    fn final_exponentiation_part_two(&self, elt: &Fp6<'a, FE, F>, elt_inv: &Fp6<'a, FE, F>) -> Fp6<'a, FE, F> {
        let mut elt_q = elt.clone();
        elt_q.frobenius_map(1);

        let mut w1_part = elt_q.cyclotomic_exp(&self.exp_w1);
        let w0_part = match self.exp_w0_is_negative {
            true => elt_inv.cyclotomic_exp(&self.exp_w0),
            false => elt.cyclotomic_exp(&self.exp_w0),
        };

        w1_part.mul_assign(&w0_part);

        w1_part
    }
}


impl<
    'a, 
        FE: ElementRepr, 
        F: SizedPrimeField<Repr = FE>, 
        CB: CurveParameters<BaseFieldElement = Fp<'a, FE, F>>,
        CTW: CurveParameters<BaseFieldElement = Fp3<'a, FE, F>>
    > PairingEngine for MNT6Instance<'a, FE, F, CB, CTW> {
    type PairingResult = Fp6<'a, FE, F>;
    type G1 = CurvePoint<'a, CB>;
    type G2 = CurvePoint<'a, CTW>;

    fn pair<'b>
        (&self, points: &'b [CurvePoint<'a, CB>], twists: &'b [CurvePoint<'a, CTW>]) -> Option<Self::PairingResult> {
            if points.len() != twists.len() {
                return None;
            }

            if !crate::features::in_gas_metering() {
                if points.len() == 0 || twists.len() == 0 {
                    return None;
                }
            }
            
            let mut pairs = Vec::with_capacity(points.len());
            for (p, q) in points.iter().zip(twists.iter()) {
                if !p.is_zero() && !q.is_zero() {
                    pairs.push((p, q));
                }
            }

            if pairs.len() == 0 {
                return Some(Fp6::one(self.fp6_extension));
            }

            // let loop_result = if self.prefer_naf {
            //     self.miller_loop_naf(&pairs[..])
            // } else {
            //     self.miller_loop(&pairs[..])
            // };  

            let loop_result = self.miller_loop(&pairs[..]);

            if loop_result.is_err() {
                return None;
            }

            let loop_result = loop_result.expect("is some");

            self.final_exponentiation(&loop_result)
        }   
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;
    use crate::field::{U320Repr, new_field};
    use crate::fp::Fp;
    use crate::traits::{FieldElement, ZeroAndOne};
    use crate::extension_towers::fp3::{Fp3, Extension3};
    use crate::extension_towers::fp6_as_2_over_3::{Extension2Over3};
    use num_traits::Num;
    use crate::weierstrass::{CurveOverFpParameters, CurveOverFp3Parameters};
    use crate::weierstrass::curve::{CurvePoint, WeierstrassCurve};
    use crate::pairings::{PairingEngine};
    use crate::test::{biguint_to_u64_vec};
    use crate::integers::MaxFieldUint;

    #[test]
    fn test_mnt6_pairing() {
        let modulus = BigUint::from_str_radix("475922286169261325753349249653048451545124878552823515553267735739164647307408490559963137", 10).unwrap();
        let base_field = new_field::<U320Repr>("475922286169261325753349249653048451545124878552823515553267735739164647307408490559963137", 10).unwrap();
        let nonres_repr = U320Repr::from(5);
        let fp_non_residue = Fp::from_repr(&base_field, nonres_repr).unwrap();

        let modulus = MaxFieldUint::from_big_endian(&modulus.to_bytes_be());

        let mut extension_3 = Extension3::new(fp_non_residue.clone());
        extension_3.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let one = Fp::one(&base_field);

        let mut fp3_non_residue = Fp3::zero(&extension_3); // non-residue is 13 + 0*u + 0*u^2
        fp3_non_residue.c0 = fp_non_residue;

        let mut extension_6 = Extension2Over3::new(fp3_non_residue);
        extension_6.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let b_fp = BigUint::from_str_radix("106700080510851735677967319632585352256454251201367587890185989362936000262606668469523074", 10).unwrap().to_bytes_be();
        let b_fp = Fp::from_be_bytes(&base_field, &b_fp, true).unwrap();

        let a_fp = Fp::from_repr(&base_field, U320Repr::from(11)).unwrap();

        let mut twist = Fp3::zero(&extension_3);
        twist.c1 = one.clone();

        let mut twist_squared = twist.clone();
        twist_squared.square();

        let mut twist_cubed = twist_squared.clone();
        twist_cubed.mul_assign(&twist);

        let mut a_fp3 = twist_squared.clone();
        a_fp3.mul_by_fp(&a_fp);

        let mut b_fp3 = twist_cubed.clone();
        b_fp3.mul_by_fp(&b_fp);

        // let scalar_field = new_field::<U320Repr>("475922286169261325753349249653048451545124879242694725395555128576210262817955800483758081", 10).unwrap();
        let group_order = BigUint::from_str_radix("475922286169261325753349249653048451545124879242694725395555128576210262817955800483758081", 10).unwrap();
        let group_order = biguint_to_u64_vec(group_order);

        let fp_params = CurveOverFpParameters::new(&base_field);
        let fp3_params = CurveOverFp3Parameters::new(&extension_3);

        let curve = WeierstrassCurve::new(&group_order.as_ref(), a_fp, b_fp, &fp_params).unwrap();
        let curve_twist = WeierstrassCurve::new(&group_order.as_ref(), a_fp3, b_fp3, &fp3_params).unwrap();

        let p_x = BigUint::from_str_radix("336685752883082228109289846353937104185698209371404178342968838739115829740084426881123453", 10).unwrap().to_bytes_be();
        let p_y = BigUint::from_str_radix("402596290139780989709332707716568920777622032073762749862342374583908837063963736098549800", 10).unwrap().to_bytes_be();

        let q_x_0 = BigUint::from_str_radix("421456435772811846256826561593908322288509115489119907560382401870203318738334702321297427", 10).unwrap().to_bytes_be();
        let q_x_1 = BigUint::from_str_radix("103072927438548502463527009961344915021167584706439945404959058962657261178393635706405114", 10).unwrap().to_bytes_be();
        let q_x_2 = BigUint::from_str_radix("143029172143731852627002926324735183809768363301149009204849580478324784395590388826052558", 10).unwrap().to_bytes_be();
        
        let q_y_0 = BigUint::from_str_radix("464673596668689463130099227575639512541218133445388869383893594087634649237515554342751377", 10).unwrap().to_bytes_be();
        let q_y_1 = BigUint::from_str_radix("100642907501977375184575075967118071807821117960152743335603284583254620685343989304941678", 10).unwrap().to_bytes_be();
        let q_y_2 = BigUint::from_str_radix("123019855502969896026940545715841181300275180157288044663051565390506010149881373807142903", 10).unwrap().to_bytes_be();

        let p_x = Fp::from_be_bytes(&base_field, &p_x, true).unwrap();
        let p_y = Fp::from_be_bytes(&base_field, &p_y, true).unwrap();

        let q_x_0 = Fp::from_be_bytes(&base_field, &q_x_0, true).unwrap();
        let q_x_1 = Fp::from_be_bytes(&base_field, &q_x_1, true).unwrap();
        let q_x_2 = Fp::from_be_bytes(&base_field, &q_x_2, true).unwrap();

        let q_y_0 = Fp::from_be_bytes(&base_field, &q_y_0, true).unwrap();
        let q_y_1 = Fp::from_be_bytes(&base_field, &q_y_1, true).unwrap();
        let q_y_2 = Fp::from_be_bytes(&base_field, &q_y_2, true).unwrap();

        let mut q_x = Fp3::zero(&extension_3);
        q_x.c0 = q_x_0;
        q_x.c1 = q_x_1;
        q_x.c2 = q_x_2;

        let mut q_y = Fp3::zero(&extension_3);
        q_y.c0 = q_y_0;
        q_y.c1 = q_y_1;
        q_y.c2 = q_y_2;

        let p = CurvePoint::point_from_xy(&curve, p_x, p_y);
        let q = CurvePoint::point_from_xy(&curve_twist, q_x, q_y);

        let x: Vec<u64> = vec![
            0xdc9a1b671660000, 0x46609756bec2a33f, 0x1eef55
        ];

        assert!(p.is_on_curve());
        assert!(q.is_on_curve());

        let engine = super::MNT6InstanceParams {
            x: &x,
            x_is_negative: true,
            exp_w0: &[0xdc9a1b671660000, 0x46609756bec2a33f, 0x1eef55],
            exp_w1: &[1u64],
            exp_w0_is_negative: true,
            base_field: &base_field,
            curve: &curve,
            curve_twist: &curve_twist,
            twist: twist,
            fp3_extension: &extension_3,
            fp6_extension: &extension_6,
            force_no_naf: false
        };

        let engine = super::MNT6Instance::from_params(engine);

        let pairing_result = engine.pair(&[p], &[q]).unwrap();

        // // let expected_c0_c0_c0 = BigUint::from_str_radix("2819105605953691245277803056322684086884703000473961065716485506033588504203831029066448642358042597501014294104502", 10).unwrap();
        
        // let expected_c0_c0_c0 = BigUint::from_str_radix("1250ebd871fc0a92a7b2d83168d0d727272d441befa15c503dd8e90ce98db3e7b6d194f60839c508a84305aaca1789b6", 16).unwrap();
        
        // println!("Res = {}", pairing_result);

        assert!(format!("{}", pairing_result.c0.c0) == "0x0000014ac12149eebffe74a1c75a7225deb91ca243c49eef01392080ff519ab6209431f81b50ec03");
    }

    #[test]
    fn test_cp6_as_mnt6_pairing() {
        use crate::field::{U832Repr};
        let modulus = BigUint::from_str_radix("22369874298875696930346742206501054934775599465297184582183496627646774052458024540232479018147881220178054575403841904557897715222633333372134756426301062487682326574958588001132586331462553235407484089304633076250782629492557320825577", 10).unwrap();
        let base_field = new_field::<U832Repr>("22369874298875696930346742206501054934775599465297184582183496627646774052458024540232479018147881220178054575403841904557897715222633333372134756426301062487682326574958588001132586331462553235407484089304633076250782629492557320825577", 10).unwrap();
        let nonres_repr = U832Repr::from(13);
        let fp_non_residue = Fp::from_repr(&base_field, nonres_repr).unwrap();

        let modulus = MaxFieldUint::from_big_endian(&modulus.to_bytes_be());

        let mut extension_3 = Extension3::new(fp_non_residue.clone());
        extension_3.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let one = Fp::one(&base_field);

        let mut fp3_non_residue = Fp3::zero(&extension_3); // non-residue is 13 + 0*u + 0*u^2
        fp3_non_residue.c0 = fp_non_residue;

        let mut extension_6 = Extension2Over3::new(fp3_non_residue);
        extension_6.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let b_fp = BigUint::from_str_radix("17764315118651679038286329069295091506801468118146712649886336045535808055361274148466772191243305528312843236347777260247138934336850548243151534538734724191505953341403463040067571652261229308333392040104884438208594329793895206056414", 10).unwrap().to_bytes_be();
        let b_fp = Fp::from_be_bytes(&base_field, &b_fp, true).unwrap();

        let a_fp = Fp::from_repr(&base_field, U832Repr::from(5)).unwrap();

        let mut twist = Fp3::zero(&extension_3);
        twist.c1 = one.clone();

        let mut twist_squared = twist.clone();
        twist_squared.square();

        let mut twist_cubed = twist_squared.clone();
        twist_cubed.mul_assign(&twist);

        let mut a_fp3 = twist_squared.clone();
        a_fp3.mul_by_fp(&a_fp);

        let mut b_fp3 = twist_cubed.clone();
        b_fp3.mul_by_fp(&b_fp);

        // let scalar_field = new_field::<U832Repr>("22369874298875696930346742206501054934775599465297184582183496627646774052458024540232479018147881220178054575403841904557897715222633333372134756426301062487682326574958588001132586331462553235407484089304633076250782629492557320825577", 10).unwrap();
        let group_order = BigUint::from_str_radix("22369874298875696930346742206501054934775599465297184582183496627646774052458024540232479018147881220178054575403841904557897715222633333372134756426301062487682326574958588001132586331462553235407484089304633076250782629492557320825577", 10).unwrap();
        let group_order = biguint_to_u64_vec(group_order);

        let fp_params = CurveOverFpParameters::new(&base_field);
        let fp3_params = CurveOverFp3Parameters::new(&extension_3);

        let curve = WeierstrassCurve::new(&group_order.as_ref(), a_fp, b_fp, &fp_params).unwrap();
        let curve_twist = WeierstrassCurve::new(&group_order.as_ref(), a_fp3, b_fp3, &fp3_params).unwrap();

        let p_x = BigUint::from_str_radix("5511163824921585887915590525772884263960974614921003940645351443740084257508990841338974915037175497689287870585840954231884082785026301437744745393958283053278991955159266640440849940136976927372133743626748847559939620888818486853646", 10).unwrap().to_bytes_be();
        let p_y = BigUint::from_str_radix("7913123550914612057135582061699117755797758113868200992327595317370485234417808273674357776714522052694559358668442301647906991623400754234679697332299689255516547752391831738454121261248793568285885897998257357202903170202349380518443", 10).unwrap().to_bytes_be();

        let q_x_0 = BigUint::from_str_radix("13426761183630949215425595811885033211332897733228446437546263564078445562454176776915160094418980045665397361295624472103734543457352048745726512354895954850428989867542989474136256025045975283415690491751906307188562464175510373683338", 10).unwrap().to_bytes_be();
        let q_x_1 = BigUint::from_str_radix("20471601555918880743198170952645906008198510944268658573129351735028343217532386920456705632337352161031960990613816401042894531220068552819818037605513359562118363589199569321421558696125646867661360498323171027455638052943806292028610", 10).unwrap().to_bytes_be();
        let q_x_2 = BigUint::from_str_radix("3905053196875761830053608605277158152930144841844497593936739534395003062685449846381431331169369910535935138116320442345524758217411779027270883193856999691582831339845600938304719916501940381093815781408183227875600753651697934495980", 10).unwrap().to_bytes_be();
        
        let q_y_0 = BigUint::from_str_radix("8567517639523571619872938228644013584947463594196306323477160496987712111576624702939472765993995586889532559039169098780892505598589581147768095093536988446010255611523736706017580686335404469207486594272103717837888228343074699140243", 10).unwrap().to_bytes_be();
        let q_y_1 = BigUint::from_str_radix("3890537069205870914984502594450293167889863914413852788876350245583932846980126025043974070704295857226211547108005650399870458089721518559480870503159804530091559886149680718531004778697982910253701559194337987238111062202037698927752", 10).unwrap().to_bytes_be();
        let q_y_2 = BigUint::from_str_radix("10936269922612615564271188303104593362724754284143779051599749016735041389483971486958818324356025479751246744831831158558101688599198721653921723013062333636402617118847009085485166284126970598561393411916461254016145116183331671450721", 10).unwrap().to_bytes_be();

        let p_x = Fp::from_be_bytes(&base_field, &p_x, true).unwrap();
        let p_y = Fp::from_be_bytes(&base_field, &p_y, true).unwrap();

        let q_x_0 = Fp::from_be_bytes(&base_field, &q_x_0, true).unwrap();
        let q_x_1 = Fp::from_be_bytes(&base_field, &q_x_1, true).unwrap();
        let q_x_2 = Fp::from_be_bytes(&base_field, &q_x_2, true).unwrap();

        let q_y_0 = Fp::from_be_bytes(&base_field, &q_y_0, true).unwrap();
        let q_y_1 = Fp::from_be_bytes(&base_field, &q_y_1, true).unwrap();
        let q_y_2 = Fp::from_be_bytes(&base_field, &q_y_2, true).unwrap();

        let mut q_x = Fp3::zero(&extension_3);
        q_x.c0 = q_x_0;
        q_x.c1 = q_x_1;
        q_x.c2 = q_x_2;

        let mut q_y = Fp3::zero(&extension_3);
        q_y.c0 = q_y_0;
        q_y.c1 = q_y_1;
        q_y.c2 = q_y_2;

        let p = CurvePoint::point_from_xy(&curve, p_x, p_y);
        let q = CurvePoint::point_from_xy(&curve_twist, q_x, q_y);

        // let x = BigUint::from_str_radix("506464946133393486072777102926336625944849939610982267859828541006717966526573193706126370441346337661774335955699621", 10).unwrap();
        // println!("X len = {}", biguint_to_u64_vec(x.clone()).len());
        // println!("{:x}", biguint_to_u64_vec(x.clone())[0]);
        let w0 = BigUint::from_str_radix("7000705447348627246181409558336018323010329260726930841638672011287206690002601216854775649561085256265269640040570922609783227469279331691880282815325569032149343779036142830666859805506518426649197067288711084398033", 10).unwrap();
        let w1 = BigUint::from_str_radix("86482221941698704497288378992285180119495364068003923046442785886272123124361700722982503222189455144364945735564951562986", 10).unwrap();
        
        let x: Vec<u64> = vec![
            0x55c5b9b57b942ae8,
            0x3d52287d3dfd424a,
            0xcf1ff9d6a543deb7,
            0x820c9c5711ceeebc,
            0x549a2d44305d20fe,
            0x50f5c131afd70235,
            0xab3596c8617c5792,
            0x830c728d80f9d78b,
            0x6a7223ee72023d07,
            0xbc5d176b746af026,
            0xe959283d8f526663,
            0xc4d2263babf8941f,
            0x3848,
        ];

        assert!(p.is_on_curve());
        assert!(q.is_on_curve());

        // in this case w1 != 1

        let engine = super::MNT6InstanceParams {
            x: &x,
            x_is_negative: false,
            exp_w0: &biguint_to_u64_vec(w0),
            exp_w1: &biguint_to_u64_vec(w1),
            exp_w0_is_negative: true,
            base_field: &base_field,
            curve: &curve,
            curve_twist: &curve_twist,
            twist: twist,
            fp3_extension: &extension_3,
            fp6_extension: &extension_6,
            force_no_naf: false,
        };

        let engine = super::MNT6Instance::from_params(engine);

        let pairing_result = engine.pair(&[p], &[q]).unwrap();

        // // let expected_c0_c0_c0 = BigUint::from_str_radix("2819105605953691245277803056322684086884703000473961065716485506033588504203831029066448642358042597501014294104502", 10).unwrap();
        
        // let expected_c0_c0_c0 = BigUint::from_str_radix("1250ebd871fc0a92a7b2d83168d0d727272d441befa15c503dd8e90ce98db3e7b6d194f60839c508a84305aaca1789b6", 16).unwrap();
        
        // println!("Res = {}", pairing_result);

        assert!(format!("{}", pairing_result.c0.c0) == "0x0000000000003621057ef03d9de232637cd9d965a98d2670da76793d38546678fb8f5f148ddc5b1cbcd74c66f8d8462197406e42e9713aa158c0a4b02e4f26c785f73a0b9027c2ff50282278d2e91afbfa6dfa584f55987f960ce39228cf56ab169ba8932adc28df");
    }


    #[test]
    fn test_ey_sw6_bis() {
        use crate::weierstrass::Group;
        use crate::field::{U768Repr};
        let modulus = BigUint::from_str_radix("6891450384315732539396789682275657542479668912536150109513790160209623422243491736087683183289411687640864567753786613451161759120554247759349511699125301598951605099378508850372543631423596795951899700429969112842764913119068299", 10).unwrap();
        let base_field = new_field::<U768Repr>("6891450384315732539396789682275657542479668912536150109513790160209623422243491736087683183289411687640864567753786613451161759120554247759349511699125301598951605099378508850372543631423596795951899700429969112842764913119068299", 10).unwrap();
        let nonres_repr = U768Repr::from(2);
        let fp_non_residue = Fp::from_repr(&base_field, nonres_repr).unwrap();

        let modulus = MaxFieldUint::from_big_endian(&modulus.to_bytes_be());

        let mut extension_3 = Extension3::new(fp_non_residue.clone());
        extension_3.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let one = Fp::one(&base_field);

        let mut fp3_non_residue = Fp3::zero(&extension_3); // non-residue is 13 + 0*u + 0*u^2
        fp3_non_residue.c0 = fp_non_residue;

        let mut extension_6 = Extension2Over3::new(fp3_non_residue);
        extension_6.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let b_fp = BigUint::from_str_radix("5428247903343207843304490009542442997117969973913823318164330064320104021081180430153151788629347606889122435541645149581251622306618937027915190165889600280602116819284463614893841480114928247406042232431052188770336204942290254", 10).unwrap().to_bytes_be();
        let b_fp = Fp::from_be_bytes(&base_field, &b_fp, true).unwrap();

        let a_fp = Fp::zero(&base_field);

        let mut twist = Fp3::zero(&extension_3);
        twist.c1 = one.clone();

        let mut twist_squared = twist.clone();
        twist_squared.square();

        let mut twist_cubed = twist_squared.clone();
        twist_cubed.mul_assign(&twist);

        let mut a_fp3 = twist_squared.clone();
        a_fp3.mul_by_fp(&a_fp);

        let mut b_fp3 = twist_cubed.clone();
        b_fp3.mul_by_fp(&b_fp);

        let group_order = BigUint::from_str_radix("258664426012969094010652733694893533536393512754914660539884262666720468348340822774968888139573360124440321458177", 10).unwrap();
        let group_order = biguint_to_u64_vec(group_order);

        let fp_params = CurveOverFpParameters::new(&base_field);
        let fp3_params = CurveOverFp3Parameters::new(&extension_3);

        let curve = WeierstrassCurve::new(&group_order.as_ref(), a_fp, b_fp, &fp_params).unwrap();
        let curve_twist = WeierstrassCurve::new(&group_order.as_ref(), a_fp3, b_fp3, &fp3_params).unwrap();

        let p_x = BigUint::from_str_radix("5579967068336365913530314810395365133780834399862286099844214670112509916823249335294962213691537977152362219416893517101994883989493084838175435762233462039369475294964563970855229509232628421518123427796600636650128165679151508", 10).unwrap().to_bytes_be();
        let p_y = BigUint::from_str_radix("3286106293257681699728332709973922945771547429394479993180335107651349218966829706081783660531045380298579922041512096833422398940161774037026391058614654750408470238881887954061918258846716359094162226636558919540815399302189033", 10).unwrap().to_bytes_be();

        let q_x_0 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        let q_x_1 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        let q_x_2 = BigUint::from_str_radix("2762333537634180879188497831180460021045024768964216860077326831768282319295307213190468027456390916014688653832658199933857161399195589808528083537985014526923490693736135387257722421714776391535494333278083320743942589072320465", 10).unwrap().to_bytes_be();
        
        let q_y_0 = BigUint::from_str_radix("6381154894827903503049202516448541497527668437176080964419566768101131101581568586920051373882212504439521771700091985568521381031252030757451435249323352844301649858691839209117065545550058565983491476754520157763374643574911714", 10).unwrap().to_bytes_be();
        let q_y_1 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        let q_y_2 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();

        let p_x = Fp::from_be_bytes(&base_field, &p_x, true).unwrap();
        let p_y = Fp::from_be_bytes(&base_field, &p_y, true).unwrap();

        let q_x_0 = Fp::from_be_bytes(&base_field, &q_x_0, true).unwrap();
        let q_x_1 = Fp::from_be_bytes(&base_field, &q_x_1, true).unwrap();
        let q_x_2 = Fp::from_be_bytes(&base_field, &q_x_2, true).unwrap();

        let q_y_0 = Fp::from_be_bytes(&base_field, &q_y_0, true).unwrap();
        let q_y_1 = Fp::from_be_bytes(&base_field, &q_y_1, true).unwrap();
        let q_y_2 = Fp::from_be_bytes(&base_field, &q_y_2, true).unwrap();

        let mut q_x = Fp3::zero(&extension_3);
        q_x.c0 = q_x_0;
        q_x.c1 = q_x_1;
        q_x.c2 = q_x_2;

        let mut q_y = Fp3::zero(&extension_3);
        q_y.c0 = q_y_0;
        q_y.c1 = q_y_1;
        q_y.c2 = q_y_2;

        let p = CurvePoint::point_from_xy(&curve, p_x, p_y);
        let q = CurvePoint::point_from_xy(&curve_twist, q_x, q_y);

        // let x = BigUint::from_str_radix("506464946133393486072777102926336625944849939610982267859828541006717966526573193706126370441346337661774335955699621", 10).unwrap();
        // println!("X len = {}", biguint_to_u64_vec(x.clone()).len());
        // println!("{:x}", biguint_to_u64_vec(x.clone())[0]);
        let w0 = BigUint::from_str_radix("2156695813352724974824326851054479880127610960548355747044807332080688727374737671308314095389122345740953981240668571898337613282699493372314698360451061276517306188376803619985090458895588556562724088277106828", 10).unwrap();
        let w1 = BigUint::from_str_radix("26642435879335816683987677701488073867751118270052650655942102502312977592501693353047140953112195348280268661194889", 10).unwrap();
        let ate_loop_length = BigUint::from_str_radix("6891450384315732539396789682275657542479668912536150109513790160209623422243491736087683183289411687640864567753786354786735746151460237106615816805591765205438850184717968966109876910955248455129124731541829539482640472797610122", 10).unwrap();

        assert!(p.is_on_curve());
        assert!(q.is_on_curve());

        assert!(p.check_correct_subgroup());
        assert!(q.check_correct_subgroup());

        // in this case w1 != 1

        let engine = super::MNT6InstanceParams {
            x: &biguint_to_u64_vec(ate_loop_length),
            x_is_negative: false,
            exp_w0: &biguint_to_u64_vec(w0),
            exp_w1: &biguint_to_u64_vec(w1),
            exp_w0_is_negative: false,
            base_field: &base_field,
            curve: &curve,
            curve_twist: &curve_twist,
            twist: twist,
            fp3_extension: &extension_3,
            fp6_extension: &extension_6,
            force_no_naf: false
        };

        let engine = super::MNT6Instance::from_params(engine);

        let mut p0 = p.clone();
        p0.double();
        p0.negate();
        p0.normalize();

        let q0 = q.clone();

        let p1 = p.clone();
        let mut q1 = q.clone();
        q1.double();
        q1.normalize();

        assert!(p0.is_on_curve());
        assert!(p1.is_on_curve());
        assert!(q0.is_on_curve());
        assert!(q1.is_on_curve());

        let mut p2 = p.mul(vec![12345678]);
        p2.normalize();

        let mut q2 = q.mul(vec![12345678]);
        q2.normalize();

        let ans1 = engine.pair(&[p.clone()], &[q2]).unwrap();
        let ans2 = engine.pair(&[p2], &[q.clone()]).unwrap();
        let ans3 = engine.pair(&[p], &[q]).unwrap();
        let ans3 = ans3.pow(&vec![12345678]);

        assert!(ans1 == ans2);
        assert!(ans1 == ans3);
    }


    #[test]
    fn test_ey_pendulum() {
        use crate::weierstrass::Group;
        use crate::field::{U640Repr};
        let modulus = BigUint::from_str_radix("19050022797317891600939264904924934656417895081121634056186244048763811669585984032184028629480644260294123843823582617865870693473572190965725707704312821545976965077621486794922414287", 10).unwrap();
        let base_field = new_field::<U640Repr>("19050022797317891600939264904924934656417895081121634056186244048763811669585984032184028629480644260294123843823582617865870693473572190965725707704312821545976965077621486794922414287", 10).unwrap();
        let nonres_repr = U640Repr::from(3);
        let fp_non_residue = Fp::from_repr(&base_field, nonres_repr).unwrap();

        let modulus = MaxFieldUint::from_big_endian(&modulus.to_bytes_be());

        let mut extension_3 = Extension3::new(fp_non_residue.clone());
        extension_3.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let one = Fp::one(&base_field);

        let mut fp3_non_residue = Fp3::zero(&extension_3);
        fp3_non_residue.c0 = fp_non_residue;

        let mut extension_6 = Extension2Over3::new(fp3_non_residue);
        extension_6.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let b_fp = BigUint::from_str_radix("3779136", 10).unwrap().to_bytes_be();
        let b_fp = Fp::from_be_bytes(&base_field, &b_fp, true).unwrap();

        let a_fp = Fp::zero(&base_field);

        let mut twist = Fp3::zero(&extension_3);
        twist.c1 = one.clone();

        let mut twist_squared = twist.clone();
        twist_squared.square();

        let mut twist_cubed = twist_squared.clone();
        twist_cubed.mul_assign(&twist);

        let mut a_fp3 = twist_squared.clone();
        a_fp3.mul_by_fp(&a_fp);

        let mut b_fp3 = twist_cubed.clone();
        b_fp3.mul_by_fp(&b_fp);

        let group_order = BigUint::from_str_radix("475922286169261325753349249653048451545124878552823515553267735739164647307408490559963137", 10).unwrap();
        let group_order = biguint_to_u64_vec(group_order);

        println!("Group order has {} words", group_order.len());

        let fp_params = CurveOverFpParameters::new(&base_field);
        let fp3_params = CurveOverFp3Parameters::new(&extension_3);

        let curve = WeierstrassCurve::new(&group_order.as_ref(), a_fp, b_fp, &fp_params).unwrap();
        let curve_twist = WeierstrassCurve::new(&group_order.as_ref(), a_fp3, b_fp3, &fp3_params).unwrap();

        let p_x = BigUint::from_str_radix("10429529130963884009088672788347332381112680208340155261267801535021515284160681139607251964825852124795332292830211013740612118630760054643157598813437729109347416973526597847768430196", 10).unwrap().to_bytes_be();
        let p_y = BigUint::from_str_radix("16905957489427084020812201420874004139445860435619110387337972510865682281055685071559898890725067816998514457554471177636644195808052025100386087604283564319339703394765085347212298098", 10).unwrap().to_bytes_be();

        let q_x_0 = BigUint::from_str_radix("13912037883745548354885384080735939137135972624544456324778703816799088874865621783205940029962037178108763091296730317064882342261056217993081826235556952843127200569255758772006603222", 10).unwrap().to_bytes_be();
        let q_x_1 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        let q_x_2 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        
        let q_y_0 = BigUint::from_str_radix("17848446077687999456842554535611770905438459561283772120727453120807955998955357528566232686711238095565974536067108118163952231407922320805847265871073335951065323478695430783920346311", 10).unwrap().to_bytes_be();
        let q_y_1 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        let q_y_2 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();

        let p_x = Fp::from_be_bytes(&base_field, &p_x, true).unwrap();
        let p_y = Fp::from_be_bytes(&base_field, &p_y, true).unwrap();

        let q_x_0 = Fp::from_be_bytes(&base_field, &q_x_0, true).unwrap();
        let q_x_1 = Fp::from_be_bytes(&base_field, &q_x_1, true).unwrap();
        let q_x_2 = Fp::from_be_bytes(&base_field, &q_x_2, true).unwrap();

        let q_y_0 = Fp::from_be_bytes(&base_field, &q_y_0, true).unwrap();
        let q_y_1 = Fp::from_be_bytes(&base_field, &q_y_1, true).unwrap();
        let q_y_2 = Fp::from_be_bytes(&base_field, &q_y_2, true).unwrap();

        let mut q_x = Fp3::zero(&extension_3);
        q_x.c0 = q_x_0;
        q_x.c1 = q_x_1;
        q_x.c2 = q_x_2;

        let mut q_y = Fp3::zero(&extension_3);
        q_y.c0 = q_y_0;
        q_y.c1 = q_y_1;
        q_y.c2 = q_y_2;

        let p = CurvePoint::point_from_xy(&curve, p_x, p_y);
        let q = CurvePoint::point_from_xy(&curve_twist, q_x, q_y);

        // let x = BigUint::from_str_radix("506464946133393486072777102926336625944849939610982267859828541006717966526573193706126370441346337661774335955699621", 10).unwrap();
        // println!("X len = {}", biguint_to_u64_vec(x.clone()).len());
        // println!("{:x}", biguint_to_u64_vec(x.clone())[0]);
        let w0 = BigUint::from_str_radix("18882571883840774900563563343436885760643859164070784837888459545889142700580885367286044189612745808185152546256306761299542795636596957234150172272931757786521125192466997795000886229", 10).unwrap();
        let w1 = BigUint::from_str_radix("40027591375585988881374325570481291274861054588565855055039992018070225508664671878855045285090", 10).unwrap();
        let ate_loop_length = BigUint::from_str_radix("19050022797317891600939264904924934656417895081121634056186244048763811669585984032184028629480168338007954582497829268616217645022027066087172884188759553810237800430314078304362451150", 10).unwrap();

        println!("Miller loop length is {} bits", ate_loop_length.bits());
        println!("Miller loop hamming is {}", crate::public_interface::decode_utils::calculate_hamming_weight(&biguint_to_u64_vec(ate_loop_length.clone())));

        println!("Exp w0 length is {} bits", w0.bits());
        println!("Exp w0 hamming is {}", crate::public_interface::decode_utils::calculate_hamming_weight(&biguint_to_u64_vec(w0.clone())));

        println!("Exp w1 length is {} bits", w1.bits());
        println!("Exp w1 hamming is {}", crate::public_interface::decode_utils::calculate_hamming_weight(&biguint_to_u64_vec(w1.clone())));
        
        assert!(p.is_on_curve());
        assert!(q.is_on_curve());

        assert!(p.check_correct_subgroup());
        assert!(q.check_correct_subgroup());

        // in this case w1 != 1

        let engine = super::MNT6InstanceParams {
            x: &biguint_to_u64_vec(ate_loop_length),
            x_is_negative: false,
            exp_w0: &biguint_to_u64_vec(w0),
            exp_w1: &biguint_to_u64_vec(w1),
            exp_w0_is_negative: false,
            base_field: &base_field,
            curve: &curve,
            curve_twist: &curve_twist,
            twist: twist,
            fp3_extension: &extension_3,
            fp6_extension: &extension_6,
            force_no_naf: false
        };

        let engine = super::MNT6Instance::from_params(engine);

        let mut p0 = p.clone();
        p0.double();
        p0.negate();
        p0.normalize();

        let q0 = q.clone();

        let p1 = p.clone();
        let mut q1 = q.clone();
        q1.double();
        q1.normalize();

        assert!(p0.is_on_curve());
        assert!(p1.is_on_curve());
        assert!(q0.is_on_curve());
        assert!(q1.is_on_curve());

        let mut p2 = p.mul(vec![12345678]);
        p2.normalize();

        let mut q2 = q.mul(vec![12345678]);
        q2.normalize();

        let ans1 = engine.pair(&[p.clone()], &[q2]).unwrap();
        let ans2 = engine.pair(&[p2], &[q.clone()]).unwrap();
        let ans3 = engine.pair(&[p], &[q]).unwrap();
        let ans3 = ans3.pow(&vec![12345678]);

        assert!(ans1 == ans2);
        assert!(ans1 == ans3);
    }

    #[test]
    fn test_sw6_curve_with_g2_in_fp() {
        use crate::weierstrass::Group;
        use crate::field::{U768Repr};
        let modulus = BigUint::from_str_radix("6891450384315732539396789682275657542479668912536150109513790160209623422243491736087683183289411687640864567753786613451161759120554247759349511699125301598951605099378508850372543631423596795951899700429969112842764913119068299", 10).unwrap();
        let base_field = new_field::<U768Repr>("6891450384315732539396789682275657542479668912536150109513790160209623422243491736087683183289411687640864567753786613451161759120554247759349511699125301598951605099378508850372543631423596795951899700429969112842764913119068299", 10).unwrap();
        let nonres_repr = U768Repr::from(2);
        // let nonres_repr = U768Repr::from(0);
        let fp_non_residue = Fp::from_repr(&base_field, nonres_repr).unwrap();

        let modulus = MaxFieldUint::from_big_endian(&modulus.to_bytes_be());

        let mut extension_3 = Extension3::new(fp_non_residue.clone());
        extension_3.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let one = Fp::one(&base_field);

        let mut fp3_non_residue = Fp3::zero(&extension_3);
        fp3_non_residue.c0 = fp_non_residue;

        let mut extension_6 = Extension2Over3::new(fp3_non_residue);
        extension_6.calculate_frobenius_coeffs_optimized(&modulus).expect("must work");

        let b_fp = BigUint::from_str_radix("5428247903343207843304490009542442997117969973913823318164330064320104021081180430153151788629347606889122435541645149581251622306618937027915190165889600280602116819284463614893841480114928247406042232431052188770336204942290254", 10).unwrap().to_bytes_be();
        let b_fp = Fp::from_be_bytes(&base_field, &b_fp, true).unwrap();

        let a_fp = Fp::zero(&base_field);

        let mut twist = Fp3::zero(&extension_3);
        twist.c1 = one.clone();

        let mut twist_squared = twist.clone();
        twist_squared.square();

        let mut twist_cubed = twist_squared.clone();
        twist_cubed.mul_assign(&twist);

        let mut a_fp3 = twist_squared.clone();
        a_fp3.mul_by_fp(&a_fp);

        let mut b_fp3 = twist_cubed.clone();
        b_fp3.mul_by_fp(&b_fp);

        let group_order = BigUint::from_str_radix("258664426012969094010652733694893533536393512754914660539884262666720468348340822774968888139573360124440321458177", 10).unwrap();
        let group_order = biguint_to_u64_vec(group_order);

        println!("Group order has {} words", group_order.len());

        let fp_params = CurveOverFpParameters::new(&base_field);
        let fp3_params = CurveOverFp3Parameters::new(&extension_3);

        let curve = WeierstrassCurve::new(&group_order.as_ref(), a_fp, b_fp, &fp_params).unwrap();
        let curve_twist = WeierstrassCurve::new(&group_order.as_ref(), a_fp3, b_fp3, &fp3_params).unwrap();

        let p_x = BigUint::from_str_radix("5579967068336365913530314810395365133780834399862286099844214670112509916823249335294962213691537977152362219416893517101994883989493084838175435762233462039369475294964563970855229509232628421518123427796600636650128165679151508", 10).unwrap().to_bytes_be();
        let p_y = BigUint::from_str_radix("3286106293257681699728332709973922945771547429394479993180335107651349218966829706081783660531045380298579922041512096833422398940161774037026391058614654750408470238881887954061918258846716359094162226636558919540815399302189033", 10).unwrap().to_bytes_be();

        let q_x_0 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        let q_x_1 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        let q_x_2 = BigUint::from_str_radix("2762333537634180879188497831180460021045024768964216860077326831768282319295307213190468027456390916014688653832658199933857161399195589808528083537985014526923490693736135387257722421714776391535494333278083320743942589072320465", 10).unwrap().to_bytes_be();
        
        let q_y_0 = BigUint::from_str_radix("6381154894827903503049202516448541497527668437176080964419566768101131101581568586920051373882212504439521771700091985568521381031252030757451435249323352844301649858691839209117065545550058565983491476754520157763374643574911714", 10).unwrap().to_bytes_be();
        let q_y_1 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();
        let q_y_2 = BigUint::from_str_radix("0", 10).unwrap().to_bytes_be();

        let p_x = Fp::from_be_bytes(&base_field, &p_x, true).unwrap();
        let p_y = Fp::from_be_bytes(&base_field, &p_y, true).unwrap();

        let q_x_0 = Fp::from_be_bytes(&base_field, &q_x_0, true).unwrap();
        let q_x_1 = Fp::from_be_bytes(&base_field, &q_x_1, true).unwrap();
        let q_x_2 = Fp::from_be_bytes(&base_field, &q_x_2, true).unwrap();

        let q_y_0 = Fp::from_be_bytes(&base_field, &q_y_0, true).unwrap();
        let q_y_1 = Fp::from_be_bytes(&base_field, &q_y_1, true).unwrap();
        let q_y_2 = Fp::from_be_bytes(&base_field, &q_y_2, true).unwrap();

        let mut q_x = Fp3::zero(&extension_3);
        q_x.c0 = q_x_0;
        q_x.c1 = q_x_1;
        q_x.c2 = q_x_2;

        let mut q_y = Fp3::zero(&extension_3);
        q_y.c0 = q_y_0;
        q_y.c1 = q_y_1;
        q_y.c2 = q_y_2;

        let p = CurvePoint::point_from_xy(&curve, p_x, p_y);
        let q = CurvePoint::point_from_xy(&curve_twist, q_x, q_y);

        // let x = BigUint::from_str_radix("506464946133393486072777102926336625944849939610982267859828541006717966526573193706126370441346337661774335955699621", 10).unwrap();
        // println!("X len = {}", biguint_to_u64_vec(x.clone()).len());
        // println!("{:x}", biguint_to_u64_vec(x.clone())[0]);
        let w0 = BigUint::from_str_radix("2156695813352724974824326851054479880127610960548355747044807332080688727374737671308314095389122345740953981240668571898337613282699493372314698360451061276517306188376803619985090458895588556562724088277106828", 10).unwrap();
        let w1 = BigUint::from_str_radix("26642435879335816683987677701488073867751118270052650655942102502312977592501693353047140953112195348280268661194889", 10).unwrap();
        let ate_loop_length = BigUint::from_str_radix("6891450384315732539396789682275657542479668912536150109513790160209623422243491736087683183289411687640864567753786354786735746151460237106615816805591765205438850184717968966109876910955248455129124731541829539482640472797610122", 10).unwrap();

        println!("Miller loop length is {} bits", ate_loop_length.bits());
        println!("Miller loop hamming is {}", crate::public_interface::decode_utils::calculate_hamming_weight(&biguint_to_u64_vec(ate_loop_length.clone())));

        println!("Exp w0 length is {} bits", w0.bits());
        println!("Exp w0 hamming is {}", crate::public_interface::decode_utils::calculate_hamming_weight(&biguint_to_u64_vec(w0.clone())));

        println!("Exp w1 length is {} bits", w1.bits());
        println!("Exp w1 hamming is {}", crate::public_interface::decode_utils::calculate_hamming_weight(&biguint_to_u64_vec(w1.clone())));
        
        assert!(p.is_on_curve());
        assert!(q.is_on_curve());

        assert!(p.check_correct_subgroup());
        assert!(q.check_correct_subgroup());

        // in this case w1 != 1

        let engine = super::MNT6InstanceParams {
            x: &biguint_to_u64_vec(ate_loop_length),
            x_is_negative: false,
            exp_w0: &biguint_to_u64_vec(w0),
            exp_w1: &biguint_to_u64_vec(w1),
            exp_w0_is_negative: false,
            base_field: &base_field,
            curve: &curve,
            curve_twist: &curve_twist,
            twist: twist,
            fp3_extension: &extension_3,
            fp6_extension: &extension_6,
            force_no_naf: false
        };

        let engine = super::MNT6Instance::from_params(engine);

        let mut p0 = p.clone();
        p0.double();
        p0.negate();
        p0.normalize();

        let q0 = q.clone();

        let p1 = p.clone();
        let mut q1 = q.clone();
        q1.double();
        q1.normalize();

        assert!(p0.is_on_curve());
        assert!(p1.is_on_curve());
        assert!(q0.is_on_curve());
        assert!(q1.is_on_curve());

        let mut p2 = p.mul(vec![12345678]);
        p2.normalize();

        let mut q2 = q.mul(vec![12345678]);
        q2.normalize();

        let ans1 = engine.pair(&[p.clone()], &[q2]).unwrap();
        let ans2 = engine.pair(&[p2], &[q.clone()]).unwrap();
        let ans3 = engine.pair(&[p], &[q]).unwrap();
        let ans3 = ans3.pow(&vec![12345678]);

        assert!(ans1 == ans2);
        assert!(ans1 == ans3);
    }
}