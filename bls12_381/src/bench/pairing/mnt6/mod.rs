extern crate test as rust_test;
use self::rust_test::Bencher;

use num_bigint::BigUint;
use num_traits::FromPrimitive;
use num_integer::Integer;
use num_traits::Zero;
use crate::field::{U320Repr, U832Repr, new_field, biguint_to_u64_vec};
use crate::fp::Fp;
use crate::traits::{FieldElement};
use crate::traits::ZeroAndOne;
use crate::extension_towers::fp3::{Fp3, Extension3};
use crate::extension_towers::fp6_as_2_over_3::{Fp6, Extension2Over3};
use num_traits::Num;
use crate::weierstrass::{Group, CurveOverFpParameters, CurveOverFp3Parameters};
use crate::weierstrass::curve::{CurvePoint, WeierstrassCurve};
use crate::pairings::{PairingEngine};
use crate::pairings::mnt6::MNT6Instance;

#[bench]
fn bench_mnt6_pairing(b: &mut Bencher) {
    let modulus = BigUint::from_str_radix("475922286169261325753349249653048451545124878552823515553267735739164647307408490559963137", 10).unwrap();
    let base_field = new_field::<U320Repr>("475922286169261325753349249653048451545124878552823515553267735739164647307408490559963137", 10).unwrap();
    let nonres_repr = U320Repr::from(5);
    let mut fp_non_residue = Fp::from_repr(&base_field, nonres_repr).unwrap();

    let mut extension_3 = Extension3::new(fp_non_residue.clone());
    extension_3.calculate_frobenius_coeffs(modulus.clone()).unwrap();

    let one = Fp::one(&base_field);

    let mut fp3_non_residue = Fp3::zero(&extension_3); // non-residue is 13 + 0*u + 0*u^2
    fp3_non_residue.c0 = fp_non_residue;

    let mut extension_6 = Extension2Over3::new(fp3_non_residue.clone());
    extension_6.calculate_frobenius_coeffs(modulus.clone()).unwrap();

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

    let curve = WeierstrassCurve::new(group_order.clone(), a_fp, b_fp, &fp_params).unwrap();
    let curve_twist = WeierstrassCurve::new(group_order.clone(), a_fp3, b_fp3, &fp3_params).unwrap();

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

    let engine = MNT6Instance {
        x: x,
        x_is_negative: true,
        exp_w0: vec![0xdc9a1b671660000, 0x46609756bec2a33f, 0x1eef55],
        exp_w1: vec![1u64],
        exp_w0_is_negative: true,
        base_field: &base_field,
        curve: &curve,
        curve_twist: &curve_twist,
        twist: twist,
        fp3_extension: &extension_3,
        fp6_extension: &extension_6,
    };

    b.iter(|| {
        engine.pair(&[p.clone()], &[q.clone()]).unwrap();
    });
}

#[bench]
fn bench_cp6_as_mnt6_pairing(b: &mut Bencher) {
    let modulus = BigUint::from_str_radix("22369874298875696930346742206501054934775599465297184582183496627646774052458024540232479018147881220178054575403841904557897715222633333372134756426301062487682326574958588001132586331462553235407484089304633076250782629492557320825577", 10).unwrap();
    let base_field = new_field::<U832Repr>("22369874298875696930346742206501054934775599465297184582183496627646774052458024540232479018147881220178054575403841904557897715222633333372134756426301062487682326574958588001132586331462553235407484089304633076250782629492557320825577", 10).unwrap();
    let nonres_repr = U832Repr::from(13);
    let mut fp_non_residue = Fp::from_repr(&base_field, nonres_repr).unwrap();

    let mut extension_3 = Extension3::new(fp_non_residue.clone());
    extension_3.calculate_frobenius_coeffs(modulus.clone()).unwrap();

    let one = Fp::one(&base_field);

    let mut fp3_non_residue = Fp3::zero(&extension_3); // non-residue is 13 + 0*u + 0*u^2
    fp3_non_residue.c0 = fp_non_residue;

    let mut extension_6 = Extension2Over3::new(fp3_non_residue.clone());
    extension_6.calculate_frobenius_coeffs(modulus.clone()).unwrap();

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

    let scalar_field = new_field::<U832Repr>("22369874298875696930346742206501054934775599465297184582183496627646774052458024540232479018147881220178054575403841904557897715222633333372134756426301062487682326574958588001132586331462553235407484089304633076250782629492557320825577", 10).unwrap();
    let group_order = BigUint::from_str_radix("22369874298875696930346742206501054934775599465297184582183496627646774052458024540232479018147881220178054575403841904557897715222633333372134756426301062487682326574958588001132586331462553235407484089304633076250782629492557320825577", 10).unwrap();
    let group_order = biguint_to_u64_vec(group_order);

    let fp_params = CurveOverFpParameters::new(&base_field);
    let fp3_params = CurveOverFp3Parameters::new(&extension_3);

    let curve = WeierstrassCurve::new(group_order.clone(), a_fp, b_fp, &fp_params).unwrap();
    let curve_twist = WeierstrassCurve::new(group_order.clone(), a_fp3, b_fp3, &fp3_params).unwrap();

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

    let engine = MNT6Instance {
        x: x,
        x_is_negative: false,
        exp_w0: biguint_to_u64_vec(w0),
        exp_w1: biguint_to_u64_vec(w1),
        exp_w0_is_negative: true,
        base_field: &base_field,
        curve: &curve,
        curve_twist: &curve_twist,
        twist: twist,
        fp3_extension: &extension_3,
        fp6_extension: &extension_6,
    };

    b.iter(|| {
        engine.pair(&[p.clone()], &[q.clone()]).unwrap();
    });
}