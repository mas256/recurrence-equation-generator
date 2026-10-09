"""Development-only comparison with the pinned Python v0.7.0 generator.
Usage: cargo run --example export-fixtures > /tmp/fixtures.json
       python scripts/check-reference.py /path/to/math-textbooks /tmp/fixtures.json [report.json]
Python and the reference checkout are never needed by the running app.
"""
import json
import sys
from pathlib import Path
from fractions import Fraction
from math import comb

reference, fixture_file = map(Path, sys.argv[1:3])
sys.path.insert(0, str(reference / "recurrence/scripts"))
from compiler import compile_blocks
from expr import num, index, nat, add, mul, sub, evaluate
from rules import load_config
from scoring import score_routes, level, assess
from solve import find_routes
from generate import compile_recipe, FAMILIES

config = load_config()
BASELINE_FAMILIES = {"arithmetic", "geometric", "scaled_constant", "factorial_ratio", "second_order", "reciprocal_affine", "weighted_sum"}
def rational(value):
    return Fraction(int(value["num"]), int(value["den"]))
def old_recipe(fixture):
    family = fixture["recipe"]["family"]
    p = {key: rational(value) for key, value in fixture["recipe"]["parameters"].items()}
    c = p["c"]
    core = {"id": "core", "kind": "constant", "parameters": {"initial": num(c)}}
    blocks = []
    if family == "arithmetic": blocks = [{"kind": "index_add", "value": mul(num(p["d"]), sub(index(), num(1)))}]
    elif family == "geometric": core = {"id": "core", "kind": "geometric", "parameters": {"amplitude": num(c), "ratio": num(p["r"])}}
    elif family == "scaled_constant": blocks = [{"kind": "index_scale", "factor": index()}]
    elif family == "factorial_ratio": blocks = [{"kind": "factorial_product", "ratio": num(p["r"]), "offsets": [0]}]
    elif family == "second_order":
        core = {"id": "core", "kind": "geometric", "parameters": {"amplitude": num(c), "ratio": num(p["r"])}}
        blocks = [{"kind": "linear_combination", "other_core": {"kind": "geometric", "initial": num(p["d"]), "ratio": num(p["s"])}}]
    elif family == "reciprocal_affine":
        core["parameters"]["initial"] = num(1/c)
        blocks = [{"kind": "index_add", "value": mul(num(p["d"]), sub(index(), num(1)))}, {"kind": "reciprocal", "requires": "positive_input"}]
    elif family == "weighted_sum":
        r, s = p["r"], p["s"]
        aa, bb = c*(r-1)/(r-s), c*(1-s)/(r-s)
        core = {"id": "core", "kind": "geometric", "parameters": {"amplitude": num(aa), "ratio": num(r)}}
        blocks = [{"kind": "linear_combination", "other_core": {"kind": "geometric", "initial": num(bb), "ratio": num(s)}}, {"kind": "index_scale", "factor": index()}, {"kind": "sum_encode"}]
    previous = "core"
    for i, b in enumerate(blocks, 1):
        b.update(id=f"b{i}", input=previous); previous = b["id"]
    return {"schema_version": "0.2", "rule_set_version": config["version"], "domain": {"index_start": 1, "sequence_type": "rational"}, "core": core, "blocks": blocks, "output": previous}

def reference_ir(fixture):
    family = fixture["recipe"]["family"]
    if family in BASELINE_FAMILIES:
        profile = {"scaled_constant":"n","factorial_ratio":"monomial","weighted_sum":"distinct:n"}.get(family,"none")
        return compile_blocks(old_recipe(fixture), config), profile
    assert family in FAMILIES, (family, "unregistered reference family")
    p = {key:rational(value) for key,value in fixture["recipe"]["parameters"].items()}
    parameters = {"r":Fraction(2),"c":Fraction(1),"d":Fraction(1),"s":Fraction(3)}
    parameters.update({key:p[key] for key in parameters if key in p})
    selector = p.get("profile",Fraction(0))
    assert selector.denominator == 1
    code = int(selector)
    scale_names = ["n","n_plus_1","n_squared","consecutive","gap_2","odd"]
    profile = "none"
    if family in {"shifted_scaled","scaled_affine","reciprocal_scaled"}:
        assert 0 <= code < 12
        profile = ("inverse:" if code >= 6 else "") + scale_names[code % 6]
    elif family in {"scaled_second_order","difference_scaled","coupled_scaled"}:
        profile = ["n","n_plus_1","odd"][code]
    elif family == "polynomial_difference": profile = scale_names[code]
    elif family in {"pure_sum","sum_relation"}:
        profile = ["constant","linear"][code]
    elif family == "pure_sum_scaled": profile = scale_names[code]
    elif family == "coupled_forced": profile = ["constant","n"][code]
    elif family == "mobius": parameters["s"] = p["h"]
    elif family == "ratio_power":
        profile = ["triangular","square_minus_one","tetrahedral"][code]
        parameters["d"] = p["c"]
    elif family == "multiplicative_second":
        profile = ["constant","n","odd"][code]
        parameters["s"] = p["e"]+2
    result = compile_recipe(family,parameters,profile,config)
    assert result["quality"]["accepted"], (family,profile,result["quality"]["reasons"])
    return result["ir"], profile

def rust_nat(expr, n, bound=None):
    kind = expr["kind"]
    if kind == "index": return n
    if kind == "constant": return expr["value"]
    if kind == "bound": return bound[expr["name"]]
    if kind == "add": return rust_nat(expr["left"], n, bound) + rust_nat(expr["right"], n, bound)
    if kind == "sub": return max(0, rust_nat(expr["left"], n, bound) - rust_nat(expr["right"], n, bound))
    if kind == "mul": return rust_nat(expr["left"],n,bound)*rust_nat(expr["right"],n,bound)
    if kind == "div": return rust_nat(expr["numerator"],n,bound)//rust_nat(expr["denominator"],n,bound)
    if kind == "pow": return rust_nat(expr["base"],n,bound)**expr["exponent"]
    if kind == "choose":
        value = rust_nat(expr["n"],n,bound)
        return comb(value,expr["k"]) if value >= expr["k"] else 0
    raise ValueError(kind)
def rust_eval(expr, n, sequence=None, bound=None):
    kind = expr["kind"]
    if kind == "rational": return rational(expr["value"])
    if kind == "nat_cast": return Fraction(rust_nat(expr["value"], n, bound))
    if kind == "term":
        return sequence(expr["sequence"],rust_nat(expr["index"], n, bound))
    if kind == "add": return sum((rust_eval(e,n,sequence,bound) for e in expr["terms"]), Fraction(0))
    if kind == "mul":
        v = Fraction(1)
        for e in expr["factors"]: v *= rust_eval(e,n,sequence,bound)
        return v
    if kind == "div": return rust_eval(expr["numerator"],n,sequence,bound)/rust_eval(expr["denominator"],n,sequence,bound)
    if kind == "pow": return rust_eval(expr["base"],n,sequence,bound)**rust_nat(expr["exponent"],n,bound)
    if kind == "factorial":
        from math import factorial
        return Fraction(factorial(rust_nat(expr["index"],n,bound)))
    if kind == "sum":
        lower, upper = rust_nat(expr["lower"],n,bound), rust_nat(expr["upper"],n,bound)
        return sum((rust_eval(expr["body"],n,sequence,{**(bound or {}),expr["variable"]:k})
                    for k in range(lower,upper+1)),Fraction(0))
    raise ValueError(kind)

def sequence_indices(expr,n,bound=None):
    kind = expr["kind"]
    if kind == "term": return [(expr["sequence"],rust_nat(expr["index"],n,bound))]
    if kind == "sum":
        lower,upper = rust_nat(expr["lower"],n,bound),rust_nat(expr["upper"],n,bound)
        return [item for k in range(lower,upper+1) for item in sequence_indices(expr["body"],n,{**(bound or {}),expr["variable"]:k})]
    children = expr.get("terms",expr.get("factors",[]))
    if kind == "div": children = [expr["numerator"],expr["denominator"]]
    if kind == "pow": children = [expr["base"]]
    return [item for child in children for item in sequence_indices(child,n,bound)]

def compare_recurrence(fixture, ir, n, scenario):
    """Compare the next-term functions on arbitrary exact input terms.

    The legacy compiler sometimes cross-multiplies its equation. Isolating
    the future term allows both representations without assuming the answer.
    This is a development cross-check; the all-index Lean proof is separate.
    """
    current = n-1  # Python reference uses internal zero-based indices.
    equations = [fixture["problem"]["recurrence"],*fixture["problem"].get("secondary_recurrences",[])]
    for row,equation in enumerate(equations):
        target = "a" if row == 0 else "b"
        indices = sequence_indices(equation["lhs"],n)+sequence_indices(equation["rhs"],n)
        future = max(index for name,index in indices if name == target)
        max_offset = max(2,future-n)
        def sequence(name,k,future_value):
            if name == target and k == future: return future_value
            return Fraction(k*k+2*k+3+scenario+(5 if name == "b" else 0),scenario+1)+Fraction(adjustment,17)
        def residual(future_value,legacy):
            if legacy:
                values = {offset:sequence("a",n+offset,future_value) for offset in range(-current,max_offset+1)}
                values.update({("b",offset):sequence("b",n+offset,future_value) for offset in range(max_offset+1)})
                values.update({("S",offset):sum((sequence("a",k,future_value) for k in range(1,n+offset+1)),Fraction(0)) for offset in range(max_offset+1)})
                lhs,rhs = (ir["lhs"],ir["rhs"]) if row == 0 else (ir["b_lhs"],ir["b_rhs"])
                return evaluate(lhs,current,values)-evaluate(rhs,current,values)
            terms = lambda name,k:sequence(name,k,future_value)
            return rust_eval(equation["lhs"],n,terms)-rust_eval(equation["rhs"],n,terms)
        def next_term(legacy):
            zero,one = residual(Fraction(0),legacy),residual(Fraction(1),legacy)
            coefficient = one-zero
            assert coefficient, "The last sequence term must have a nonzero coefficient"
            return -zero/coefficient
        # A fractional recurrence has genuine poles on arbitrary inputs. Select
        # another exact input at those poles instead of assuming the answer.
        for adjustment in range(32):
            try:
                rust_next,reference_next = next_term(False),next_term(True)
            except ZeroDivisionError:
                continue
            assert rust_next == reference_next, (fixture["recipe"]["family"],n,scenario,target,"recurrence")
            break
        else:
            raise AssertionError((fixture["recipe"]["family"],n,target,"no admissible arbitrary input"))

fixtures = json.loads(fixture_file.read_text())
cases = []
for fixture in fixtures:
    ir,profile = reference_ir(fixture)
    family = fixture["recipe"]["family"]
    problem = fixture["problem"]
    assert problem["index_start"] == problem["recurrence_start"] == 1
    assert problem["sequence_type"] == "rational"
    system = ir.get("shape") == "system"
    expected_initials = [*ir["initials"],*([ir["b_initial"]] if system else [])]
    assert len(problem["initials"]) == len(expected_initials), (family,"initial count")
    def reference_term(name,k): return evaluate(ir["b_formula"] if name == "b" else ir["formula"],k-1)
    for position,initial in enumerate(problem["initials"]):
        expected = evaluate(expected_initials[position],0)
        assert rust_eval(initial["rhs"],1) == expected, (family,position,"initial value")
        assert rust_eval(initial["lhs"],1,reference_term) == expected, (family,position,"initial boundary")
    secondary = problem.get("secondary_general_terms",[])
    assert len(secondary) == int(system), (family,"secondary formula count")
    assert len(problem.get("secondary_recurrences",[])) == int(system), (family,"secondary recurrence count")
    formula = fixture["problem"]["general_term"]["rhs"]
    for n in range(1,21):
        assert rust_eval(formula,n) == evaluate(ir["formula"],n-1), (family,n,"formula")
        if system: assert rust_eval(secondary[0]["rhs"],n) == evaluate(ir["b_formula"],n-1), (family,n,"secondary formula")
        for scenario in range(3):
            compare_recurrence(fixture,ir,n,scenario)
    routes = score_routes(ir, find_routes(ir,config))
    assert routes, (family,"reference forward solver found no complete route")
    assert routes[0]["complete"], (family,"reference route is incomplete")
    quality = assess(ir, routes, config)
    assert quality["accepted"], (family,profile,"reference quality",quality["reasons"])
    assert routes[0]["difficulty_cost"] == fixture["classification"]["score"], (family,routes[0]["difficulty_cost"],"educational score")
    assert level(routes[0]["difficulty_cost"]) == fixture["classification"]["level"], (family,"level")
    cases.append({"family":family,"reference_profile":profile,"level":fixture["classification"]["level"],"score":fixture["classification"]["score"]})
report = {"reference_version":"0.7.0","reference_commit":"a21c3d0711ebb08d0516e646f32ee3cab7b2c7de","case_count":len(cases),"family_count":len({case["family"] for case in cases}),"registered_profile_count":len({(case["family"],case["reference_profile"]) for case in cases}),"quality_checked":True,"cases":cases}
if len(sys.argv) > 3: Path(sys.argv[3]).write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
print(f"PASS: {len(fixtures)} cases / {report['family_count']} families; exact initial boundaries, terms n=1..20, all recurrence rows on three arbitrary inputs per index, and educational scores against Python v0.7.0")
