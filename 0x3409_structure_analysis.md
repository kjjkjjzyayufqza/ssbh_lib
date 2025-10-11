# 0x3409 数据结构完整分析

## 关键发现：多层嵌套的对象结构

### 结构体布局（基于 sub_1402423F0）

```c
struct AnimationTrackProcessor {
    // 前面的字段（0-63）
    int field_00;                          // +0x00
    // ... 其他字段 ...
    
    // 关键指针字段（在函数开始时初始化为0）
    void* field_64;                        // +0x40 (64)  - 初始化为0
    int field_72;                          // +0x48 (72)  - 初始化为0
    void* field_136;                       // +0x88 (136) - 初始化为0  
    void* field_200;                       // +0xC8 (200) - 初始化为0
    
    // 函数对象区域
    FunctionObject interpolator;           // +0x50 (80)  - 由 sub_1402381A0 创建
    //  虚表: _lambda_7c80d3bccf2818536ef2f57a848ddbdb_
    //  签名: void(void*, void const*, void const*, float)
    
    // Lambda 对象存储区域
    LambdaStorage lambda_storage;          // +0x90 (144) - 传递给 sub_1402430B0
};
```

### sub_1402423F0 的执行流程

```c
__int64 sub_1402423F0(__int64 a1, __int64 a2)
{
    // 1. 初始化关键字段为0
    *(_DWORD *)a1 = 0;
    *(_QWORD *)(a1 + 64) = 0i64;
    *(_DWORD *)(a1 + 72) = 0;
    *(_QWORD *)(a1 + 136) = 0i64;
    *(_QWORD *)(a1 + 200) = 0i64;
    
    // 2. 调用初始化函数，设置 a1+72 和 a1+80
    sub_140236CC0(a1, a2+384, a2+392);
    //   设置 a1+72 为归一化的帧时间
    //   在 a1+80 创建插值函数对象
    
    // 3. 根据条件选择不同的Lambda
    if (a2+40 == a2+80) {
        // 简单情况：常量值
        // 创建简单Lambda（虚表: _lambda_9fb3499d1afc91a542260beae21b169f_）
        // 存储到 a1+144
    }
    else {
        // 复杂情况：0x3409 压缩
        // 提取 88 字节数据 (a2+32 到 a2+120)
        // 调用 sub_1402430B0(a1+144, 88字节数据)
        //   -> sub_140244800 创建 Lambda 对象
        //   -> 虚表: _lambda_66c459b58a37c3d862655d970fbecbc0_
    }
    
    return a1;
}
```

### Lambda 对象在 a1+144 的存储

`sub_1402430B0` 实际上是设置 `a1+144` 区域的函数对象。它调用 `sub_140244800` 创建 104 字节的 Lambda 对象，然后通过 `sub_140039FE0` 将其移动/复制到 `a1+144`。

### sub_140235330 中的对象使用（已解决）

**更新：** `a1` 是 `AnimationTrackProcessor` (Decompressor Context) 指针。

```c
void sub_140235330(__int64 a1, __int64 a2, float a3)
{
    // a1 是 AnimationTrackProcessor 指针
    // a1+64 是 read_single_frame 对象指针 (Read Single Frame / RSF)
    // a1+136 是 interpolate_between_frames 对象指针 (Interpolate Between Frames / IBF)
    
    // Read Single Frame (RSF) Logic VCall:
    v6 = *(_QWORD *)(a1 + 64);  // 获取 RSF 对象指针
    (*(__int64 (__fastcall **)(__int64, char **, float *))
        (*(_QWORD *)v6 + 16i64))(v6, &v12, &v11); // 调用 RSF::operator()
    
    // ...
    
    // Interpolate Between Frames (IBF) Logic VCall:
    v10 = *(_QWORD *)(a1 + 136);  // 获取 IBF 对象指针
    (*(__int64 (__fastcall **)(__int64, __int64 *, char **, char **, float *))
        (*(_QWORD *)v10 + 16i64))(v10, &v14, &v13, &v12, &v11); // 调用 IBF::operator()
}
```

## 关键发现：核心逻辑定位（已解决）

**1. Keyframe 数据**
- **发现：** Keyframes (K1, K2, K3) 是 3 个 Vector3，共 9 个浮点数 (36 字节)，存储在原始数据结构的 Keyframe 区域。

**2. 配置参数**
- **时间归一化因子：** 位于 `AnimationTrackProcessor` (Decompressor Context) 结构的 **+72 偏移** (`sub_140236CC0` 赋初始值)。

**3. BitReader/分段插值实现 (RSF Core)**
- **RSF Core 转发函数：** RSF 对象的 `operator()` 最终调用 **`sub_14023FCF0`**。
- **分量处理：** `sub_14023FCF0` 负责 X, Y, Z, W 四个分量的分派，它通过其输入结构中的四个函数指针 (`a1[4]`、`a1[15]`、`a1[26]`、`a1[37]`)，调用最终的 BitReader/分段插值逻辑。
- **IBF Core：** IBF 对象的 `operator()` 执行简单的线性插值。
