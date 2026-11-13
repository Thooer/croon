# Tree

Tree 是提供者一侧共用的内容格式，Kernel 不解释它。它是 Bundle 的一种用法，不是 Kernel 的机制。

Tree 是目录 Bundle：内容为名字到条目的映射，直接依赖恰好覆盖 file、dir 指向的 Bundle，排序去重。文件 Bundle 只封装字节，依赖为空；目录递归遵循相同规则。link 是目标字符串，不引入依赖。

形状与磁盘分开：Tree 由内容与依赖决定，校验只看这两样。目录怎样遍历、符号链接跟不跟、可执行位怎么记、路径穿越怎么拒绝，取决于宿主与调用方，不改变 Tree 是什么。所以 `croon-tree` 不碰文件系统；把它读出来或写下去是使用方的事。

```json directory.tree
{
  "type":"object",
  "description":"名字非空，不含 / 或 NUL，不是 . 或 ..",
  "additionalProperties":{"$ref":"#/$defs/entry"},
  "$defs":{
    "entry":{
      "anyOf":[
        {"type":"object","properties":{"file":{"type":"string","description":"字节 Bundle ID"},"x":{"type":"boolean"}},"required":["file","x"],"additionalProperties":false},
        {"type":"object","properties":{"dir":{"type":"string","description":"目录 Bundle ID"}},"required":["dir"],"additionalProperties":false},
        {"type":"object","properties":{"link":{"type":"string"}},"required":["link"],"additionalProperties":false}
      ]
    }
  }
}
```

参考：Kernel/README.md；Kernel/Value.md（Bundle）。
