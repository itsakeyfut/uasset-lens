# uasset-lens 概要

## 概要

Unreal Engine プロジェクト向けの Asset / Blueprint 静的解析・可視化・監査ツールを Rust で開発する。

本ツールは、巨大化した Unreal プロジェクトにおける以下の課題を解決することを目的とする。

- Asset 依存関係の不透明化
- Blueprint の巨大化
- 未使用 Asset の蓄積
- Circular Dependency
- Git diff の困難さ
- Asset 管理コスト増大
- Package/Cook サイズ肥大化
- チーム開発時のレビュー困難

単なる Asset Viewer ではなく、

> Unreal Project Observability Platform

として設計する。

---

## 想定ユーザー

- Unreal Engine 開発者
- インディーゲーム開発者
- Technical Artist
- Tools Programmer
- Gameplay Programmer
- 大規模 Unreal プロジェクトを扱うチーム

---

## 解決したい課題

### Asset Explosion

Unreal プロジェクト巨大化に伴い以下の問題が発生する。

- 何がどこから参照されているかわからない
- 不要 Asset が削除できない
- Blueprint の循環参照
- 巨大 BP 化
- Redirector 地獄
- Asset Rename の恐怖
- Build/Cook 時間増加
- Package サイズ肥大化

### Blueprint Black Box 問題

Blueprint は GUI ベースのため:

- grep 不可能
- diff 困難
- review 困難
- static analysis 困難

その結果、巨大化すると保守性が大きく低下する。

---

## ソフトウェアコンセプト

### コンセプト

"Clippy for Unreal Assets"

### 提供価値

- Asset 可視化
- Asset 健康状態分析
- Blueprint 静的解析
- Dependency Graph
- Lint
- Git Friendly Analysis

### 重要視する点

- 高速
- 並列解析
- CLI First
- CI Integration
- Git Friendly
- Cross Platform
- Large Project Friendly

### 非機能要件

| 要件 | 目標値 |
|---|---|
| メモリ使用量 | 100 MB 以内（UE5 + VS と共存するため） |
| スキャン速度 | 1,000 assets を 5 秒以内（フルスキャン時、並列化前提） |
| 対応最大規模 | 100,000 assets まで |

#### 設計上の制約

- 全 Asset を一度にメモリに展開しない（ストリーミング・チャンク処理）
- 大規模データは SQLite に委譲し、メモリ上のグラフは必要な範囲のみ保持
- 並列スキャンにより速度目標を達成する（rayon 使用）

---

## コア思想

### Engine Replacement は目指さない

以下はスコープ外:

- 独自ゲームエンジン
- Unreal Replacement
- 汎用 Engine 開発

目指す方向:

> Unreal Engine 開発を強化する

---

## CLI First 方針

初期段階では GUI より CLI を優先する。

理由:

- 実装速度
- CI Integration
- OSS Friendly
- Automation Friendly
- Large Project Friendly

---

## 競合・関連分野

### 既存 Unreal の課題

Unreal は:

- Asset 可視化が弱い
- BP diff が弱い
- 大規模 Asset 管理が難しい
- GUI 依存が強い

本ツールはそこを補完する。

---

## 最終ビジョン

本プロジェクトの最終目標は:

> Unreal プロジェクトを "見える化" すること

である。

特に:

- Asset
- Blueprint
- Dependency
- Complexity
- Project Health

を可視化し、巨大 Unreal プロジェクトの保守性を向上させる。
