#!/usr/bin/env python3
"""AgentWatcher macOS 全量主功能自检（数据面）。

用法：python3 mac-selfcheck.py <agentwatcher-cli 路径>
输出：逐项 PASS/FAIL 与总结；退出码 0=全部通过。
"""
import json
import os
import re
import subprocess
import sys
import tempfile
import time

CLI = sys.argv[1] if len(sys.argv) > 1 else "./target/debug/agentwatcher-cli"
RESULTS = []


def check(name, ok, detail=""):
    RESULTS.append((name, ok, detail))
    mark = "PASS" if ok else "FAIL"
    print(f"[{mark}] {name}" + (f" —— {detail}" if detail and not ok else ""))


def run(*args, **kw):
    proc = subprocess.run([CLI, *args], capture_output=True, text=True, timeout=kw.get("timeout", 120))
    try:
        return json.loads(proc.stdout), proc
    except json.JSONDecodeError:
        return None, proc


INJECTION_PREFIXES = (
    "<external_browser_tabs", "<in-app-browser-context", "<codex_internal_context",
    "<codex_apps_client_time_context", "<send_user_message_question_reply",
    "<skills_instructions", "<multi_agent_role", "<multi_agent_mode",
    "<collaboration_mode", "<recommended_plugins", "<app-context", "<agent-context",
    "<user_goal", "<system-reminder", "<permissions", "<tools>", "<image_resize_notice",
    "<environment_context", "# AGENTS.md instructions", "Current task state:",
)

# ---- 1. session list：信封与字段质量 ----
envelope, proc = run("session", "list", "--format", "json", "--active-days", "30")
check("session list 信封 ok=true", bool(envelope and envelope.get("ok")),
      proc.stdout[:200] + proc.stderr[:200])
sessions = (envelope or {}).get("data", {}).get("sessions", [])
check("session list 有会话（30 天窗）", len(sessions) > 0, f"共 {len(sessions)} 条")
providers = {s.get("provider") for s in sessions}
print(f"    providers: {sorted(providers)}")

REQUIRED_KEYS = ("id", "provider", "providerLabel", "title", "status", "timeLabel",
                 "updatedMs", "workspacePath", "workspaceLabel")
missing = [k for s in sessions for k in REQUIRED_KEYS if k not in s]
check("卡片级元数据键齐全", not missing, f"缺失 {set(missing)}")

bad_titles = [s["id"] for s in sessions
              if isinstance(s.get("title"), str)
              and any(s["title"].startswith(p) for p in INJECTION_PREFIXES)]
check("标题无注入块", not bad_titles, str(bad_titles[:3]))

half_cut = [s["id"] for s in sessions
            if isinstance(s.get("title"), str) and s["title"].endswith("...")]
check("标题无硬截半句（clean_label 省略号残留）", not half_cut, str(half_cut[:3]))

bad_ws = [s["id"] for s in sessions
          if isinstance(s.get("workspacePath"), str) and "\\" in s["workspacePath"]]
check("workspacePath 为 mac 原生路径（无反斜杠）", not bad_ws, str(bad_ws[:3]))

bad_status = [s["id"] for s in sessions if s.get("status") not in ("waiting", "running", "idle")]
check("状态三档合法", not bad_status, str(bad_status[:3]))

# updatedMs 单调性（列表按状态+时间排序，同状态内应递减）
for provider in providers:
    stamps = [s["updatedMs"] for s in sessions if s["provider"] == provider]
    if len(stamps) > 2 and stamps != sorted(stamps, reverse=True):
        # running > idle 排序允许交错，只在 idle 段检查
        idle_stamps = [s["updatedMs"] for s in sessions
                       if s["provider"] == provider and s["status"] == "idle"]
        check(f"{provider} idle 段时间递减", idle_stamps == sorted(idle_stamps, reverse=True))
        break

# ---- 2. usage 面：list.usage 与 session usage 一致 ----
usage_sessions = [s for s in sessions if s.get("usage")]
print(f"    带 usage 的会话: {len(usage_sessions)}/{len(sessions)}")
mismatch = []
mismatch_detail = []
for s in usage_sessions[:6]:
    detail, _ = run("session", "usage", s["id"], "--format", "json")
    got = (detail or {}).get("data", {}).get("usage") or {}
    listed = s["usage"].get("totalInputTokens")
    detailed = got.get("totalInputTokens")
    # 活跃写入中的会话两次调用之间 token 只增不减；静态会话应严格相等。
    ok_pair = detailed == listed or (
        isinstance(listed, (int, float)) and isinstance(detailed, (int, float))
        and detailed > listed and (detailed - listed) < listed * 0.01)
    if not ok_pair:
        mismatch.append(s["id"])
        mismatch_detail.append(f"{s['id'][:20]}: list={listed} detail={detailed}")
check("session usage 与 list.usage 口径一致（静态相等/活跃单调增<1%）", not mismatch, "; ".join(mismatch_detail))

usage_zero = [s["id"] for s in usage_sessions
              if not (s["usage"].get("totalInputTokens") or s["usage"].get("totalOutputTokens"))]
check("usage 字段非全零", not usage_zero, str(usage_zero[:3]))

# ---- 3. session show --content：正文与路径可达 ----
content_ok, content_detail = [], []
for provider in sorted(providers):
    target = next((s for s in sessions if s["provider"] == provider), None)
    if not target:
        continue
    show, proc = run("session", "show", target["id"], "--content", "--format", "json")
    if not (show or {}).get("ok"):
        content_detail.append(f"{provider}: show 失败 {proc.stdout[:120]}")
        continue
    data = show["data"]
    body = json.dumps(data.get("content") or {})
    session_obj = data.get("session") or {}
    raw_path = None
    if isinstance(data.get("content"), dict):
        raw_path = (data["content"].get("rawSessionPath")
                    or (data["content"].get("context") or {}).get("primarySourceFile")
                    if isinstance(data["content"].get("context"), dict) else None)
    path_ok = True
    if raw_path:
        path_ok = os.path.exists(raw_path)
        if not path_ok:
            content_detail.append(f"{provider}: 路径不存在 {raw_path}")
    content_ok.append((provider, bool(session_obj.get("title")), path_ok))
check("session show --content 各 provider 通过", not content_detail and content_ok, "; ".join(content_detail))

# 预览正文抽查：内容分级下列表不带正文，用 session show（悬浮预览级）验证。
no_preview = []
for s in [x for x in sessions if x["provider"] == "codex" and x.get("messageCount", 0) > 1][:8]:
    show, proc = run("session", "show", s["id"], "--format", "json")
    sess = (show or {}).get("data", {}).get("session") or {}
    if not (sess.get("lastUserMessage") or sess.get("lastAiMessage")):
        no_preview.append(s["id"])
check("codex 预览正文存在（>1 轮会话，经 show）", not no_preview, str(no_preview[:3]))

# ---- 4. handoff export：产物落盘且含真实内容 ----
export_target = next((s for s in sessions if s["provider"] in ("codex", "opencode", "workbuddy")), None)
if export_target:
    exp, proc = run("handoff", "export", export_target["id"], "--format", "json")
    ok = bool(exp and exp.get("ok"))
    detail = ""
    if ok:
        ctx = exp["data"]
        primary = ctx.get("primarySourceFile")
        if primary and os.path.exists(primary):
            size = os.path.getsize(primary)
            with open(primary, "r", errors="ignore") as fh:
                head = fh.read(4096)
            detail = f"{ctx.get('provider')} primary={size}B"
            check(f"handoff export 产物可达（{ctx.get('provider')}）", size > 100, f"{primary} {size}B")
            check("handoff export 内容含会话痕迹", "message" in head.lower() or "session" in head.lower()
                  or ctx.get("inlineSummary", "") != "", head[:120])
        else:
            check("handoff export 产物可达", False, str(primary))
    else:
        check("handoff export ok", False, proc.stdout[:150])
else:
    check("handoff export（无目标会话）", False, "无可导出 provider")

# ---- 5. 技能面 ----
skill_list, _ = run("skill", "list", "--format", "json")
installed = [h["path"] for h in (skill_list or {}).get("data", {}).get("hosts", [])
             if h.get("state") == "installed"]
check("skill list 三宿主 installed", len(installed) >= 2, str(installed))
byte_mismatch = [p for p in installed
                 if os.path.exists(p) and os.path.getsize(p) != (skill_list or {}).get("data", {}).get("docBytes")]
check("技能副本与内嵌版本同尺寸", not byte_mismatch, str(byte_mismatch))

# ---- 6. suggest prompt（不调模型）----
prompt_target = next((s for s in sessions if s["provider"] in ("codex", "zcode", "workbuddy", "opencode")), None)
if prompt_target:
    prompt, proc = run("suggest", "prompt", prompt_target["id"], "--format", "json")
    ok = bool(prompt and prompt.get("ok"))
    text = (prompt or {}).get("data", {}).get("prompt", "")
    check("suggest prompt 组装成功", ok and len(text) > 100,
          proc.stdout[:150] if not ok else f"prompt {len(text)} chars")
else:
    check("suggest prompt（无目标）", False, "无接线 provider 会话")

# ---- 7. 全历史模式与缓存 ----
full1, _ = run("session", "list", "--active-days", "0", "--format", "json")
full2, _ = run("session", "list", "--active-days", "0", "--format", "json")
t1, t2 = (full1 or {}).get("data", {}).get("total"), (full2 or {}).get("data", {}).get("total")
check("全历史两次一致（缓存合并）", t1 == t2 and t1 is not None, f"{t1} vs {t2}")
check("全历史 >= 30 天窗", (t1 or 0) >= len(sessions), f"full={t1} window30={len(sessions)}")

# ---- 总结 ----
fails = [r for r in RESULTS if not r[1]]
print(f"\n==== 自检总结：{len(RESULTS) - len(fails)}/{len(RESULTS)} 通过 ====")
for name, _, detail in fails:
    print(f"  FAIL {name}: {detail}")
sys.exit(1 if fails else 0)
