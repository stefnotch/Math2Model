import {
  idToVariableName,
  NodeReturn,
  nodeToVariableDeclaration,
  reteSocket,
  VPNode,
} from "@/vpnodes/basic/nodes";
import { SliderControl } from "@/vpnodes/controls/slider";
import { ClassicPreset } from "rete";
import { vec2, vec3 } from "webgpu-matrix";
import { type SerializedNode } from "@/vpnodes/serialization/node";
import {
  typeToValue,
  typeToValueCode,
  valueToType,
} from "@/vpnodes/basic/functions";

export class CombineNode extends VPNode {
  private cfControl: SliderControl;
  constructor(
    private update: (id: string) => void,
    private updateControl: (c: ClassicPreset.Control) => void,
  ) {
    super("Combine Shapes");

    this.cfControl = new SliderControl(
      0.5,
      1.0,
      0.0,
      0.01,
      "Combine Factor",
      false,
      (value) => {
        this.update(this.id);
      },
      (value) => {
        this.updateControl(this.cfControl);
      },
    );

    this.addInput(
      "param1",
      new ClassicPreset.Input(reteSocket, "shape 1/vec3f"),
    );
    this.addInput(
      "param2",
      new ClassicPreset.Input(reteSocket, "shape 2/vec3f"),
    );
    this.addControl("cfactor", this.cfControl);
    this.addOutput(
      "value",
      new ClassicPreset.Output(reteSocket, "output[x, y, z]/vec3f"),
    );
  }

  data(inputs: { param1: NodeReturn[]; param2: NodeReturn[] }): {
    value: NodeReturn;
  } {
    const { param1, param2 } = inputs;
    const p1 = param1 ? param1[0].refId : "vec3f(0.0, 0.0, 0.0)";
    const p2 = param2 ? param2[0].refId : "vec3f(0.0, 0.0, 0.0)";
    console.log(this.cfControl.value);
    return {
      value: {
        value: vec3.zero(),
        code: `${nodeToVariableDeclaration(this)} = mix(${p1}, ${p2}, ${this.cfControl.value});`,
        refId: idToVariableName(this.id),
      },
    };
  }

  serialize(sn: SerializedNode): SerializedNode {
    sn.nodeType = "Combine";
    sn.extraNumberInformation = [
      { key: "cf", value: this.cfControl.value ?? 0 },
    ];
    return super.serialize(sn);
  }

  deserialize(sn: SerializedNode) {
    super.deserialize(sn);
    if (sn.extraNumberInformation) {
      for (let info of sn.extraNumberInformation) {
        if (info.key == "cf") {
          this.cfControl.value = info.value;
        }
      }
    }
  }
}

export class MathFunctionNode extends VPNode {
  private variableControls: Map<string, SliderControl> = new Map();
  constructor(
    private name: string,
    private func: string,
    private update: (id: string) => void,
    private updateControl: (c: ClassicPreset.Control) => void,
  ) {
    super(`Apply ${name} Function`);

    this.setup();

    this.addInput(
      "param",
      new ClassicPreset.Input(reteSocket, "param / vec3f"),
    );

    this.addOutput(
      "value",
      new ClassicPreset.Output(reteSocket, "result / vec3f"),
    );
  }

  private setup() {
    this.label = `Apply ${this.name} Function`;
    const vars = this.func.split("{");
    for (let variable of vars) {
      const expr = variable.split("}")[0].split(",");
      if (expr.length < 6) continue;
      if (this.hasControl(expr[0])) continue;
      const control = new SliderControl(
        parseFloat(expr[1]),
        parseFloat(expr[2]),
        parseFloat(expr[3]),
        parseFloat(expr[4]),
        expr[0],
        true,
        (value) => this.update(this.id),
        (value) => this.updateControl(control),
      );
      this.variableControls.set(
        "{" + variable.split("}")[0] + "}/" + expr[5],
        control,
      );
      this.addControl(expr[0], control);
    }

    this.extraHeightControls = 40;
    this.updateSize();
  }

  data(inputs: { param: NodeReturn[] }): { value: NodeReturn } {
    let funcCall = this.func;
    let { param } = inputs;

    funcCall = funcCall.replaceAll(
      "input2",
      String(param ? param[0].refId ?? param[0].value : "input2"),
    );
    for (let key of this.variableControls.keys()) {
      const k = key.split("/");
      const control = this.variableControls.get(key);
      if (k[1] === "same") {
        funcCall = funcCall.replaceAll(
          k[0],
          typeToValueCode(
            valueToType(param ? param[0].value : vec2.create(0.0, 0.0)),
            control?.value ?? 0.0,
            control?.value ?? 0.0,
            control?.value ?? 0.0,
            control?.value ?? 0.0,
          ),
        );
      } else {
        funcCall = funcCall.replaceAll(
          k[0],
          typeToValueCode(
            k[1],
            control?.value ?? 0.0,
            control?.value ?? 0.0,
            control?.value ?? 0.0,
            control?.value ?? 0.0,
          ),
        );
      }
    }

    return {
      value: {
        value: param ? param[0].value : 0.0,
        code: `${nodeToVariableDeclaration(this)}_1 = ${funcCall};\n${nodeToVariableDeclaration(this)} = vec3f(${param ? param[0].refId + ".x" : 0.0}, ${idToVariableName(this.id)}_1.x * ${idToVariableName(this.id)}_1.y, ${param ? param[0].refId + ".z" : 0.0});`,
        refId: idToVariableName(this.id),
      },
    };
  }

  serialize(sn: SerializedNode): SerializedNode {
    sn.nodeType = "MathFunction";
    sn.extraStringInformation = [
      { key: "name", value: this.name },
      { key: "func", value: this.func },
    ];
    sn.extraNumberInformation = [];
    for (let key of this.variableControls.keys()) {
      sn.extraNumberInformation.push({
        key: key,
        value: this.variableControls.get(key)?.value ?? 0.0,
      });
    }
    return super.serialize(sn);
  }

  deserialize(sn: SerializedNode) {
    if (sn.extraStringInformation) {
      for (let info of sn.extraStringInformation) {
        if (info.key === "name") {
          this.name = info.value;
        }
        if (info.key === "func") {
          this.func = info.value;
        }
      }
    }
    this.setup();
    if (sn.extraNumberInformation) {
      for (let info of sn.extraNumberInformation) {
        if (this.variableControls.has(info.key)) {
          const cont = this.variableControls.get(info.key);
          if (cont) cont.value = info.value;
        }
      }
    }
    super.deserialize(sn);
  }
}
