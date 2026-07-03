# 知识范围标准

知识库必须先按 scope 隔离，再按用途分类。

scope 类型：

- `engine_core`
- `engine_plugin`
- `project_plugin`
- `project`
- `global`

团队知识库建议结构：

```text
UnrealWorkflowKnowledge/
  kb-registry.yaml
  schemas/
  global/
    rules/
    workflows/
    index/
  scopes/
    engines/
      ue-5.5.4/
        engine-core/
        engine-plugins/<plugin>/
    projects/
      <project>/
        project/
        plugins/<plugin>/
```

规则：

- 不同插件知识不能混放。
- 项目插件不能和引擎核心混放。
- 引擎内插件必须使用独立 `engine_plugin` scope。
- 本机重型缓存不进 git。
- 团队 Git 只提交稳定 Markdown、scope manifest、schema、轻量索引。
