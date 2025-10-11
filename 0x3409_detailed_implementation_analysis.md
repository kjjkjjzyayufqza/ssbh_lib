# 0x3409 压缩算法详细实现分析

## 当前已确认的信息

### 1. 数据流程图

```
3409.bin 压缩数据
    ↓
sub_1402407B0 (主调度器)
    ↓
sub_140241A20 (解析组件名称)
    ↓
sub_1402423F0 (Vector3 解码器入口) ← a2 参数包含压缩轨道的完整信息
    ↓
    ├─ sub_140236CC0 (初始化，设置 a1+72 等字段)
    ↓
    └─ sub_1402430B0 (数据绑定) ← 传递 88 字节结构
        ↓
        sub_140244800 (Lambda 构造) ← 创建 104 字节对象
            ↓
            设置虚表 0x141AB82D0
            ↓
            捕获 88 字节数据到 Lambda 内部
```

### 2. sub_1402423F0 提取的 88 字节数据结构

从反编译代码可见，`a2` 参数的布局：

```c
struct CompressedTrackData {
    void* field_00;              // +0x00 (8 bytes)  - v8[0] = *(void**)a2
    // ... 其他字段 ...
    __int64 field_32;            // +0x20 (8 bytes)  - v9 = *(a2+32)
    __int128 field_40_56;        // +0x28 (16 bytes) - v10 = *(a2+40)
    __int128 field_56_72;        // +0x38 (16 bytes) - v11 = *(a2+56)
    __int64 field_72;            // +0x48 (8 bytes)  - v12 = *(a2+72)
    __int128 field_80_96;        // +0x50 (16 bytes) - v13 = *(a2+80)
    __int128 field_96_112;       // +0x60 (16 bytes) - v14 = *(a2+96)
    __int64 field_112;           // +0x70 (8 bytes)  - v15 = *(a2+112)
    
    // 特殊字段（用于判断分支）
    int field_40_value;          // +0x28 - 用于判断是否相等
    int field_80_value;          // +0x50 - 用于判断是否相等
    
    __int64 field_384;           // +0x180 - 传递给 sub_140236CC0
    __int64 field_392;           // +0x188 - 传递给 sub_140236CC0
};
```

**关键发现**：
- 如果 `field_40_value == field_80_value`，创建简单 Lambda
- 否则，提取 88 字节（偏移 32-120）创建完整 Lambda

### 3. Lambda 对象结构（104 字节）

```c
struct Lambda_0x3409 {
    void* vtable;                      // +0x00 (8 bytes)  - 0x141AB82D0
    __int64 captured_08;               // +0x08 (8 bytes)  - 从 v8
    __int64 captured_16;               // +0x10 (8 bytes)  - 从 v9
    __int128 captured_24_40;           // +0x18 (16 bytes) - 从 v10
    __int128 captured_40_56;           // +0x28 (16 bytes) - 从 v11
    __int64 captured_56;               // +0x38 (8 bytes)  - 从 v12
    void* read_frame_func;             // +0x40 (8 bytes)  - 偏移+64 函数指针
    __int128 captured_72_88;           // +0x48 (16 bytes) - 从 v13
    __int64 captured_88;               // +0x58 (8 bytes)  - 未知来源?
    void* interpolate_func;            // +0x88 (8 bytes)  - 偏移+136 函数指针
    // 剩余字节...
};
```

### 4. 虚表函数（0x141AB82D0）

```assembly
+0x00: sub_140244400  - 析构函数
+0x08: sub_1402443A0  - 拷贝/移动函数
+0x10: sub_140244330  - operator() 核心解压缩入口 ✓ 已分析
+0x18: sub_140244320  - 辅助虚函数
+0x20: sub_140243940  - 其他虚函数
```

### 5. 核心插值逻辑（sub_140235330）✓ 已完全理解

```c
void interpolate_frame(__int64 lambda_obj, __int64 output, float frame_time)
{
    int frame_index = (int)frame_time;
    float fractional = frame_time - frame_index;
    
    // 边界优化
    if (fractional < 0.01f) {
        read_single_frame(lambda_obj+64, output, frame_index);
        return;
    }
    
    if (fractional > 0.99f) {
        read_single_frame(lambda_obj+64, output, frame_index + 1);
        return;
    }
    
    // 常规插值
    char frame_1[44];
    char frame_2[52];
    
    read_single_frame(lambda_obj+64, frame_1, frame_index);
    read_single_frame(lambda_obj+64, frame_2, frame_index + 1);
    
    interpolate_between_frames(lambda_obj+136, output, frame_1, frame_2, fractional);
}
```

## 待解决的关键问题 (已解决)

### 问题 1: 核心解压缩和位流逻辑 (`read_single_frame`) 是什么？

**发现**：
- 实际的单帧读取逻辑 (RSF) 封装在 `AnimationSourceDecompressor` 对象内，并通过 `sub_140235330` 调用其虚函数 (`+64` 偏移处对象的虚表 +16)。
- RSF 核心逻辑位于 **`sub_14023FCF0`** 及其内部调用的 **四个函数指针**，分别负责 X, Y, Z, W 分量的 BitReader 和分段插值。
- Keyframe 数据 (3 x Vector3) 和时间归一化参数已定位。

### 问题 2: 核心线性插值逻辑 (`interpolate_between_frames`) 是什么？

**发现**：
- 线性插值逻辑 (IBF) 类似于 RSF，通过 Decompressor 对象偏移 `+136` 处的对象虚函数 (`+16`) 调用。
- IBF 负责对 `sub_14023FCF0` 返回的两个 Vector3 结果进行简单的线性插值。

### 问题 3: 88 字节数据中 Keyframe 和 配置参数的位置在哪？

**发现**：
- **Keyframe 数据**：Keyframe 确实由 K1, K2, K3 (3 个 Vector3) 组成，共 9 个浮点数 (36 字节) 存储在原始数据结构的 Keyframe 区域。
- **配置参数**：时间缩放因子（用于归一化时间）位于 `AnimationSourceDecompressor` 结构体 **+72 偏移处**。

## 下一步行动计划 (已完成分析，转为实现)

### 优先级 1: 编写 Python 重构
- 基于已确认的 **分段线性插值数学模型** 和 **Decompressor 结构**，开始实现 `Decompressor0x3409` 类。

### 优先级 2: 确定 BitReader 细节
- 假设 X/Y/Z/W 分量 BitReader 函数实现是相同的，并将 BitReader 逻辑（读取 21 位索引，反量化）整合到 Python 实现中。

### 优先级 3: 验证数据布局
- 使用实际的 3409.bin 数据对 Python 实现进行端到端测试，验证 Keyframe 数据布局和参数是否准确。
