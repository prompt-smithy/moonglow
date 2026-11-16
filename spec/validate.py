#!/usr/bin/env python3
"""Validate the Moonglow behavioural spec.

Checks that the settings schema is a valid JSON Schema and that every golden
vector file parses, has the expected shape, and is internally consistent.

Run from anywhere: python3 spec/validate.py
"""

import json
import sys
from pathlib import Path

SPEC = Path(__file__).resolve().parent
VECTORS = SPEC / "vectors"
EPSILON = 1e-6

errors: list[str] = []


def fail(message: str) -> None:
    errors.append(message)


def load(path: Path):
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fail(f"{path.relative_to(SPEC.parent)}: invalid JSON: {exc}")
        return None


def check_schema() -> None:
    path = SPEC / "settings.schema.json"
    schema = load(path)
    if schema is None:
        return
    try:
        import jsonschema
    except ImportError:
        print("jsonschema not installed - skipping meta-schema check", file=sys.stderr)
        return
    try:
        jsonschema.Draft202012Validator.check_schema(schema)
    except jsonschema.exceptions.SchemaError as exc:
        fail(f"settings.schema.json: not a valid JSON Schema: {exc.message}")

    if schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
        fail("settings.schema.json: $schema must be the 2020-12 dialect")

    props = schema.get("properties", {})
    for key in ("dayColorTempK", "nightColorTempK", "morningStart", "morningEnd",
                "eveningStart", "eveningEnd", "hardwareBrightnessEnabled",
                "softwareBrightnessEnabled", "splitBrightnessControls",
                "manualHardwareBrightness", "manualSoftwareBrightness"):
        if key not in props:
            fail(f"settings.schema.json: missing property {key!r}")

    for key in ("hardwareBrightnessEnabled", "softwareBrightnessEnabled", "splitBrightnessControls"):
        prop = props.get(key, {})
        if prop.get("type") != "boolean" or not isinstance(prop.get("default"), bool):
            fail(f"settings.schema.json: {key} must define a boolean default")
    for key in ("manualHardwareBrightness", "manualSoftwareBrightness"):
        prop = props.get(key, {})
        default = 80 if key == "manualHardwareBrightness" else 100
        if (prop.get("type") != "integer" or prop.get("default") != default
                or prop.get("minimum") != 1 or prop.get("maximum") != 100):
            fail(f"settings.schema.json: {key} must be an integer in [1, 100] with its documented default")


def check_kelvin() -> None:
    path = VECTORS / "kelvin.json"
    data = load(path)
    if data is None:
        return
    for key in ("description", "clamp", "vectors"):
        if key not in data:
            fail(f"kelvin.json: missing {key!r}")
            return
    lo, hi = data["clamp"]["minKelvin"], data["clamp"]["maxKelvin"]
    if not lo < hi:
        fail("kelvin.json: clamp minKelvin must be below maxKelvin")
    seen = set()
    for row in data["vectors"]:
        for key in ("kelvin", "red", "green", "blue"):
            if key not in row:
                fail(f"kelvin.json: row missing {key!r}: {row}")
                return
        k = row["kelvin"]
        if not lo <= k <= hi:
            fail(f"kelvin.json: kelvin {k} outside clamp [{lo}, {hi}]")
        if k in seen:
            fail(f"kelvin.json: duplicate kelvin {k}")
        seen.add(k)
        for channel in ("red", "green", "blue"):
            value = row[channel]
            if not 0.0 - EPSILON <= value <= 1.0 + EPSILON:
                fail(f"kelvin.json: {channel} {value} at {k} K is outside [0, 1]")
    if not seen:
        fail("kelvin.json: no vectors")


def check_schedule() -> None:
    path = VECTORS / "schedule.json"
    data = load(path)
    if data is None:
        return
    for key in ("description", "step", "settings", "cases"):
        if key not in data:
            fail(f"schedule.json: missing {key!r}")
            return
    step = data["step"]
    profiles = data["settings"]
    if "default" not in profiles:
        fail("schedule.json: settings must include a 'default' profile")
    for name, profile in profiles.items():
        night, day = profile["nightColorTempK"], profile["dayColorTempK"]
        if night > day:
            fail(f"schedule.json: profile {name!r} has night {night} above day {day}")
        for window in ("morningStart", "morningEnd", "eveningStart", "eveningEnd"):
            hour, minute = profile[window]
            if not (0 <= hour < 24 and 0 <= minute < 60):
                fail(f"schedule.json: profile {name!r} {window} is not a valid time")
    for case in data["cases"]:
        for key in ("settings", "time", "kelvin"):
            if key not in case:
                fail(f"schedule.json: case missing {key!r}: {case}")
                return
        if case["settings"] not in profiles:
            fail(f"schedule.json: case references unknown profile {case['settings']!r}")
        if len(case["time"]) != 5 or case["time"][2] != ":":
            fail(f"schedule.json: case time {case['time']!r} is not HH:mm")
        kelvin = case["kelvin"]
        if kelvin % step != 0:
            fail(f"schedule.json: case kelvin {kelvin} is not on the {step} K grid")


def check_temperature_range() -> None:
    path = VECTORS / "temperature-range.json"
    data = load(path)
    if data is None:
        return
    for key in ("description", "kelvinRange", "step", "snap", "range"):
        if key not in data:
            fail(f"temperature-range.json: missing {key!r}")
            return
    lo, hi = data["kelvinRange"]["min"], data["kelvinRange"]["max"]
    step = data["step"]
    for row in data["snap"]:
        output = row["output"]
        if not lo <= output <= hi:
            fail(f"temperature-range.json: snap output {output} outside [{lo}, {hi}]")
        if output % step != 0:
            fail(f"temperature-range.json: snap output {output} is not on the {step} K grid")
        # Inputs already inside the range may only move by half a step; out-of-range
        # inputs are expected to clamp all the way to an edge.
        if lo <= row["input"] <= hi and abs(row["output"] - row["input"]) > step / 2:
            fail(f"temperature-range.json: snap moved {row['input']} to {row['output']}, "
                 f"more than half a step")
    # The range vectors deliberately include rows whose inputs are swapped or out of
    # bounds; the contract is that the outputs always come back ordered and in range.
    for row in data["range"]:
        for key in ("night", "day", "nightOut", "dayOut"):
            if key not in row:
                fail(f"temperature-range.json: range row missing {key!r}: {row}")
                return
        for key in ("nightOut", "dayOut"):
            if not lo <= row[key] <= hi:
                fail(f"temperature-range.json: {key} {row[key]} outside [{lo}, {hi}]")
            if row[key] % step != 0:
                fail(f"temperature-range.json: {key} {row[key]} is not on the {step} K grid")
        if row["nightOut"] > row["dayOut"]:
            fail(f"temperature-range.json: outputs not ordered: "
                 f"{row['nightOut']} > {row['dayOut']}")


def check_settings() -> None:
    data = load(VECTORS / "settings.json")
    if data is None:
        return
    for row in data["cases"]:
        expected = row["expected"]
        for key in ("manualColorTempK", "dayColorTempK", "nightColorTempK"):
            if key in expected and (not 900 <= expected[key] <= 6600 or expected[key] % 100):
                fail(f"settings.json: {row['name']} has an invalid {key}")
        time_keys = ("morningStart", "morningEnd", "eveningStart", "eveningEnd")
        if all(key in expected for key in time_keys):
            times = [expected[key] for key in time_keys]
        else:
            times = []
        if times and not times[0] < times[1] <= times[2] < times[3]:
            fail(f"settings.json: {row['name']} has an invalid schedule order")
        if ("nightColorTempK" in expected and "dayColorTempK" in expected
                and expected["nightColorTempK"] > expected["dayColorTempK"]):
            fail(f"settings.json: {row['name']} has inverted targets")
        for key in ("hardwareBrightnessEnabled", "softwareBrightnessEnabled", "splitBrightnessControls"):
            if not isinstance(expected.get(key), bool):
                fail(f"settings.json: {row['name']} has an invalid {key}")
        for key in ("manualBrightness", "manualHardwareBrightness", "manualSoftwareBrightness"):
            value = expected.get(key)
            if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= 100:
                fail(f"settings.json: {row['name']} has an invalid {key}")

    defaults = next((row for row in data["cases"] if row["name"] == "brightness-defaults"), None)
    if defaults is None:
        fail("settings.json: missing brightness-defaults vector")
    else:
        expected_defaults = {
            "hardwareBrightnessEnabled": True,
            "softwareBrightnessEnabled": True,
            "splitBrightnessControls": False,
            "manualHardwareBrightness": 80,
            "manualSoftwareBrightness": 100,
        }
        for key, value in expected_defaults.items():
            if defaults["expected"].get(key) != value:
                fail(f"settings.json: brightness-defaults has an incorrect {key}")


def check_brightness_routing() -> None:
    data = load(VECTORS / "brightness-routing.json")
    if data is None:
        return
    if data.get("softwareBrightnessFloor") != 50:
        fail("brightness-routing.json: softwareBrightnessFloor must be 50")
    if not data.get("vectors") or not data.get("transitions"):
        fail("brightness-routing.json: routing and transition vectors are required")

    def clamp(value):
        return min(100, max(1, value))

    def rounded(numerator, denominator):
        return (2 * numerator + denominator) // (2 * denominator)

    def route(master, split, hw, sw, ha, sa, recovery):
        floor = 1 if recovery else 50
        if split:
            return (clamp(hw) if ha else None, max(clamp(sw), floor) if sa else 100)
        master = clamp(master)
        if ha and sa:
            if master >= 50:
                return (1 + rounded((master - 50) * 99, 50), 100)
            return (1, floor + rounded((master - 1) * (99 if recovery else 49), 49 if recovery else 48))
        return (master if ha else None, max(master, floor) if sa and not ha else 100)

    def inverse(hw, sw, ha, sa, recovery):
        if ha and sa:
            hw, sw = clamp(hw), max(clamp(sw), 1 if recovery else 50)
            product = hw * sw
            if product > 100:
                return 50 + rounded((product - 100) * 50, 9900)
            shade = max(product, 1 if recovery else 50)
            if shade >= 100:
                return 50
            return 1 + rounded((shade - (1 if recovery else 50)) * (49 if recovery else 48), 99 if recovery else 49)
        return clamp(hw) if ha else clamp(sw) if sa else 80

    names = set()
    for row in data.get("vectors", []):
        name = row.get("name")
        required = ("master", "hardwareInput", "softwareInput", "split", "hardwareAvailable", "softwareAvailable", "recoveryRegistered", "expectedHardware", "expectedSoftware")
        if not name or name in names or any(key not in row for key in required):
            fail(f"brightness-routing.json: duplicate name or missing routing field: {name}")
            continue
        names.add(name)
        integers = ("master", "hardwareInput", "softwareInput", "expectedSoftware")
        booleans = ("split", "hardwareAvailable", "softwareAvailable", "recoveryRegistered")
        if any(type(row[k]) is not int for k in integers) or any(type(row[k]) is not bool for k in booleans):
            fail(f"brightness-routing.json: {name} invalid input type")
            continue
        if row["expectedHardware"] is not None and type(row["expectedHardware"]) is not int:
            fail(f"brightness-routing.json: {name} invalid hardware output type")
            continue
        expected = route(row["master"], row["split"], row["hardwareInput"], row["softwareInput"], row["hardwareAvailable"], row["softwareAvailable"], row["recoveryRegistered"])
        if (row["expectedHardware"], row["expectedSoftware"]) != expected:
            fail(f"brightness-routing.json: {name} inconsistent routing output, expected {expected}")

    for row in data.get("transitions", []):
        name = row.get("name")
        flags = ("hardwareAvailable", "softwareAvailable", "recoveryRegistered")
        if not name or name in names or any(type(row.get(k)) is not bool for k in flags):
            fail(f"brightness-routing.json: invalid transition: {name}")
            continue
        names.add(name)
        ha, sa, recovery = (row[k] for k in flags)
        if "expectedMaster" in row:
            if type(row["expectedMaster"]) is not int:
                fail(f"brightness-routing.json: {name} invalid master output type")
                continue
            if type(row.get("software")) is not int or (row.get("hardware") is not None and type(row["hardware"]) is not int):
                fail(f"brightness-routing.json: {name} invalid output inputs")
                continue
            hw, sw = row.get("hardware"), row["software"]
            if row.get("preserveOutput"):
                hw = 100 if hw is None else hw
                result = inverse(hw, sw, ha, sa, recovery) if ha and sa else max(clamp(rounded(hw * sw, 100)), 50 if sa and not recovery else 1)
            elif hw is not None:
                result = inverse(hw, sw, ha, sa, recovery)
            else:
                fail(f"brightness-routing.json: {name} missing hardware")
                continue
            if row["expectedMaster"] != result:
                fail(f"brightness-routing.json: {name} inconsistent transition, expected {result}")
        elif type(row.get("master")) is int:
            hw, sw = route(row["master"], False, 80, 100, ha, sa, recovery)
            if (row.get("expectedHardware"), row.get("expectedSoftware")) != (80 if hw is None else hw, sw):
                fail(f"brightness-routing.json: {name} inconsistent split conversion")
        else:
            fail(f"brightness-routing.json: {name} missing transition input")


def main() -> int:
    check_schema()
    check_kelvin()
    check_schedule()
    check_temperature_range()
    check_settings()
    check_brightness_routing()

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        print(f"\n{len(errors)} problem(s) in the spec", file=sys.stderr)
        return 1

    print("spec OK: schema and golden vectors are consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
