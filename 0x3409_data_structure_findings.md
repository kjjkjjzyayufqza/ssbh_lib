# 0x3409 数据结构分析结果

## 执行日期
2025-01-11

## 关键发现

### 1. 数据布局确认

通过分析实际的 0x3409 压缩数据，我们确认了以下结构：

```
Offset 0-19:   Header (20 bytes)
  - Magic: 0x3409
  - Frame count: 40
  - Unknown1: 1.0
  - Unknown2: 28.874197
  - Flags: 2
  - Bits per entry: 21

Offset 20-55:  Keyframes (36 bytes = 3 * Vector3)
  - Keyframe 0: (0.0, 0.0, 0.795940)
  - Keyframe 1: (0.0, 0.0, 71.693703)
  - Keyframe 2: (0.0, 0.0, 90.674797)

Offset 56+:    Compressed bitstream (120 bytes)
  - Contains per-frame indices
  - Repeating pattern: 0x1208 at offsets 62 and 70
```

### 2. "矩阵"的真相

**重要结论**：在 offset 0 和 offset 20 发现的"合理浮点矩阵"实际上**不是**用于 `sub_1400C7EA0` 的 4x4 变换矩阵。

这些区域实际上是：
- **Header + Keyframes 数据**：包含配置参数和关键帧值
- **Keyframes 数据**：纯粹的关键帧存储

测试结果显示，将这些"矩阵"传递给 `sub_1400C7EA0` 后：
- 输出值与预期的插值曲线**完全不匹配**
- 例如 t=0.0 时，期望 Z=0.7959，但输出为 0.0000

### 3. 压缩数据段结构

压缩数据有明显的分段头部模式：
- **重复模式 0x1208**：出现在 offset 62 和 70
  - 高字节 0x12 = 18（可能是位宽）
  - 低字节 0x08 = 8（可能是其他参数）
- 这与 `0x3409_compression_analysis.md` 中 `3409_3.bin` 的观察一致

### 4. 插值算法的本质

根据 `0x3409_compression_analysis.md` Line 50-68：

```
算法：分段线性插值（Segmented Linear Interpolation）

Segment 1 (t ∈ [0.0, 0.5]):
  local_t = t * 2.0
  result = first + (middle - first) * local_t

Segment 2 (t ∈ [0.5, 1.0]):
  local_t = (t - 0.5) * 2.0
  result = middle + (last - middle) * local_t
```

**关键洞察**：这是**简单的分段线性插值**，而不是 Hermite 样条！

### 5. sub_1400C7EA0 的角色困惑

问题：`sub_1400C7EA0` 实现了复杂的 SIMD 矩阵-向量乘法和 `_mm_blend_ps` 混合，这远超简单线性插值的需求。

**可能的解释**：

#### 假设 A：sub_1400C7EA0 不是用于这个 0x3409 压缩的
- 它可能用于其他更复杂的压缩格式（例如 Level 3 的非线性插值）
- 0x3409 使用的是更简单的插值逻辑

#### 假设 B：存在矩阵构建函数
- `sub_1400C7EA0` 需要的矩阵是**动态生成**的
- 生成函数可能：
  - 根据 keyframes 构建 Hermite 基矩阵
  - 根据配置参数（unknown2 = 28.874197）生成控制矩阵
  - 将分段线性插值参数编码进矩阵中

#### 假设 C：Level 3 转换是可选的
- 简单的轨道使用分段线性插值
- 复杂的轨道使用 `sub_1400C7EA0` + 动态矩阵
- `sub_1402423F0` 中的条件分支决定使用哪种方法

## 下一步行动

### 优先级 1：验证简单插值假设
创建一个 **不使用** `sub_1400C7EA0` 的简单解压缩器：
1. 从 bitstream 读取索引
2. 转换为 t 参数（t = index / (2^bits - 1)）
3. 应用分段线性插值
4. 与 JSON 真值对比

### 优先级 2：查找矩阵构建函数
如果简单插值失败，则反编译：
1. `sub_1402423F0` 中条件分支的两个路径
2. `sub_1402430B0` 和 `sub_140244800` 中的矩阵处理
3. 寻找可能构建 Hermite/Bezier 矩阵的代码

### 优先级 3：分析 88 字节结构
详细映射 `sub_1402423F0` 提取的 88 字节结构：
- 识别每个字段的作用
- 找到可能隐藏矩阵参数的位置

## 参考文档
- `0x3409_compression_analysis.md`: 格式规范和分段线性插值算法
- `0x3409_detailed_implementation_analysis.md`: 88 字节结构和函数调用流程
- `3409_analysis_difficulties.md`: Level 3 转换和矩阵加载的讨论
- `sub_1400C7EA0.md`: SIMD 函数的详细分析

