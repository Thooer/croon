# 两个 Seat 的 Charter

场景：最小的 Charter，检验实例化一次事件里写下整张接线图。门占 Signature 所在的 Seat，它的一
个 Required port 接 Collective 的 Required port；审计者的一个 Required port 接门的 Provided
port。实例化之后两个成员都停在自己的取上：这一份只检验诞生，成员的结束与 Collective 的收尾见
../Instance.md 待定的终止闭环。

构造与实例化各查几条，见 ../Definition/README.md 的「检查」：构造查的是这份 Charter 自己的数
据，「不比来源宽」在那里做；实例化查「占座的与 Seat 的 Interface 相同」。所以这份预期里，门的
Required port 写的声明比来源窄一点也过得了。

```json scenario.charter
{
  "values": {
    "sha256:dgate": {
      "$bundle": {
        "language": "fake@1",
        "interface": {
          "description": "门：把候选交给审计者",
          "signature": {"in": {},"out": {}},
          "provides": {"pass": {"$block": "scenario.charter#/parts/pass"}},
          "requires": {"up": {"$block": "scenario.charter#/parts/up"}}
        },
        "body": "gate"
      },
      "dependencies": []
    },
    "sha256:daudit": {
      "$bundle": {
        "language": "fake@1",
        "interface": {
          "description": "审计者：读门的 Journal，回一个结论",
          "signature": {"in": {},"out": {}},
          "provides": {},
          "requires": {"see": {"$block": "scenario.charter#/parts/pass"}}
        },
        "body": "audit"
      },
      "dependencies": []
    },
    "sha256:dcharter": {
      "$bundle": {
        "interface": {
          "description": "两个 Seat 的 Charter",
          "signature": {"in": {},"out": {}},
          "provides": {},
          "requires": {"up": {"$block": "scenario.charter#/parts/up"}}
        },
        "seats": {
          "gate": {"interface": {"$block": "scenario.charter#/parts/gate.iface"}},
          "audit": {"interface": {"$block": "scenario.charter#/parts/audit.iface"}}
        },
        "wire": {
          "gate": {"up": {"outer": "up"}},
          "audit": {"see": {"seat": "gate","port": "pass"}}
        },
        "export": {"signature": "gate","provides": {}}
      },
      "dependencies": [
        "sha256:dgate",
        "sha256:daudit"
      ]
    },
    "sha256:dcharterroot": {
      "$bundle": {
        "evaluator": "external",
        "interface": {
          "description": "这份场景的 root entry",
          "signature": {"in": {},"out": {}},
          "provides": {"up": {"$block": "scenario.charter#/parts/up"}},
          "requires": {
            "construct": {"$block": "reference.kernel#/construct"},
            "instantiate": {"$block": "reference.kernel#/instantiate"},
            "up": {"$block": "scenario.charter#/parts/up"}
          }
        }
      },
      "dependencies": []
    },
    "sha256:ccharter": {"definition": "sha256:dcharter"},
    "sha256:cg": {"definition": "sha256:dgate"},
    "sha256:ca": {"definition": "sha256:daudit"},
    "sha256:spawnc": {
      "definition": "sha256:dcharter",
      "input": {"value": null,"dependencies": []},
      "seats": {
        "gate": {"definition": "sha256:dgate","input": {"value": null,"dependencies": []}},
        "audit": {"definition": "sha256:daudit","input": {"value": null,"dependencies": []}}
      },
      "self": {}
    }
  },
  "parts": {
    "root.born": [
      {
        "by": "outside",
        "what": {
          "birth": {
            "definition": "sha256:dcharterroot",
            "wire": {"construct": "h-rconstruct","instantiate": "h-rinst","up": "h-rup"},
            "self": {"up": "port/up"},
            "input": {"value": "sha256:null","dependencies": ["sha256:dcharter","sha256:dgate","sha256:daudit"]},
            "binding": {
              "sources": {
                "construct": {"kernel": "construct","spec": {"$block": "reference.kernel#/construct"}},
                "instantiate": {"kernel": "instantiate","spec": {"$block": "reference.kernel#/instantiate"}}
              }
            }
          }
        }
      }
    ],
    "up": {"description": "Collective 的 Required port，门的 Required port 也指向它","in": {"type":"null"},"attach": {"additionalProperties": false}},
    "pass": {"description": "门交给审计者的","in": {"type": "null"},"attach": {"additionalProperties": false}},
    "gate.iface": {
      "description": "门",
      "signature": {"in": {},"out": {}},
      "provides": {"pass": {"$block": "scenario.charter#/parts/pass"}},
      "requires": {"up": {"$block": "scenario.charter#/parts/up"}}
    },
    "audit.iface": {
      "description": "审计者",
      "signature": {"in": {},"out": {}},
      "provides": {},
      "requires": {"see": {"$block": "scenario.charter#/parts/pass"}}
    },
    "spawned": {
      "outcome": {"ok": true,"value": null,"dependencies": []},
      "attach": {
        "look": {"handle": "h-clook","spec": {"$block": "reference.kernel#/look"}},
        "notify": {"handle": "h-cnotify","spec": {"$block": "reference.kernel#/notify"}},
        "correct": {"handle": "h-ccorrect","spec": {"$block": "reference.kernel#/correct"}}
      },
      "derived": {"up": {"handle": "h-cup","revoker": {"handle": "h-rev21","spec": {"$block": "reference.kernel#/revoke"}}}},
      "accepted": true
    }
  },
  "journals": {
    "i-root": [
      {"$block": "scenario.charter#/parts/root.born"},
      {
        "by": "self",
        "what": {
          "send": {
            "key": "cg",
            "to": {"port": "construct"},
            "ref": "h-rconstruct",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": {"id": "sha256:dgate"},"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:cg","dependencies": ["sha256:dgate"]}
          }
        }
      },
      {
        "by": "self",
        "what": {
          "send": {
            "key": "ca",
            "to": {"port": "construct"},
            "ref": "h-rconstruct",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": {"id": "sha256:daudit"},"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:ca","dependencies": ["sha256:daudit"]}
          }
        }
      },
      {
        "by": "self",
        "what": {
          "send": {
            "key": "cc",
            "to": {"port": "construct"},
            "ref": "h-rconstruct",
            "attach": {},
            "result": {"outcome": {"ok": true,"value": {"id": "sha256:dcharter"},"dependencies": []},"attach": {},"derived": {},"accepted": true},
            "delivery": {"value": "sha256:ccharter","dependencies": ["sha256:dcharter"]}
          }
        }
      },
      {
        "by": "self",
        "event": "e-cborn",
        "what": {
          "send": {
            "key": "spawnc",
            "to": {"port": "instantiate"},
            "ref": "h-rinst",
            "attach": {"up": {"to": {"port": "up"},"ref": "h-rup"}},
            "result": {"$block": "scenario.charter#/parts/spawned"},
            "delivery": {"value": "sha256:spawnc","dependencies": ["sha256:dcharter","sha256:dgate","sha256:daudit"]}
          }
        }
      }
    ],
    "i-c": [
      {
        "by": "i-root",
        "event": "e-cborn",
        "what": {
          "birth": {
            "definition": "sha256:dcharter",
            "wire": {"up": "h-cup"},
            "input": {"value": "sha256:null","dependencies": []},
            "staffing": {
              "gate": {"definition": "sha256:dgate","input": {"value": "sha256:null","dependencies": []}},
              "audit": {"definition": "sha256:daudit","input": {"value": "sha256:null","dependencies": []}}
            }
          }
        }
      }
    ],
    "i-gate": [
      {
        "by": "i-root",
        "event": "e-cborn",
        "what": {
          "birth": {
            "definition": "sha256:dgate",
            "wire": {"up": "h-gup"},
            "input": {"value": "sha256:null","dependencies": []},
            "collective": "i-c",
            "seat": "gate"
          }
        }
      }
    ],
    "i-audit": [
      {
        "by": "i-root",
        "event": "e-cborn",
        "what": {
          "birth": {
            "definition": "sha256:daudit",
            "wire": {"see": "h-asee"},
            "input": {"value": "sha256:null","dependencies": []},
            "collective": "i-c",
            "seat": "audit"
          }
        }
      }
    ]
  }
}
```

参考：README.md（格式、脚手架）；../Definition/README.md（Charter、Seat、检查）；../Instance.md（成员的身份）。
