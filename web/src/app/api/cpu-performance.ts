// 检测设备类型
function detectDeviceType() {
    const userAgent = navigator.userAgent.toLowerCase();
    return /android|webos|iphone|ipad|ipod|blackberry|iemobile|opera mini/i.test(userAgent);
}

// 检测CPU核心数
function detectCPUCores() {
    return navigator.hardwareConcurrency || '未知';
}

export enum CpuPerformanceLevel {
    High = '高',
    Middle = '中',
    Low = '低'
}

// 评估CPU性能
function evaluateCPUPerformance(): CpuPerformanceLevel {
    const cores = detectCPUCores();
    const isMobile = detectDeviceType();

    // 简单性能评估逻辑
    let performance = CpuPerformanceLevel.Low;
    if (cores !== '未知') {
        if (cores >= 8) {
            performance = CpuPerformanceLevel.High;
        } else if (cores >= 4) {
            performance = CpuPerformanceLevel.Middle;
        }
    }

    // 移动设备默认性能较低
    if (isMobile) {
        performance = CpuPerformanceLevel.Low;
    }

    return performance;
}

// 计算斐波那契数列（递归实现，性能较差，适合基准测试）
function fibonacci(n: number): number {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}

// 运行基准测试
async function runBenchmark() {
    // 根据设备类型调整测试强度
    const isMobile = detectDeviceType();
    const fibNumber = isMobile ? 35 : 40; // 移动设备计算较小的数

    try {
        // 运行基准测试
        const startTime = performance.now();
        fibonacci(fibNumber);
        const endTime = performance.now();

        const computationTime = endTime - startTime;
        console.log('计算斐波那契数列耗时：' + computationTime.toFixed(2) + ' ms');

        // 根据计算时间评估性能
        // 注意：这些阈值可能需要根据实际情况调整
        let performanceLevel: CpuPerformanceLevel;

        if (computationTime < 100) {
            performanceLevel = CpuPerformanceLevel.High;
        } else if (computationTime < 500) {
            performanceLevel = CpuPerformanceLevel.Middle;
        } else {
            performanceLevel = CpuPerformanceLevel.Low;
        }

        return performanceLevel;
    } catch (error) {
        console.log( '基准测试出错: ', error);
        return CpuPerformanceLevel.Low;
    }
}

export const performanceInfo = {
    level: CpuPerformanceLevel.Low
}

if(evaluateCPUPerformance() === CpuPerformanceLevel.High){
    runBenchmark().then(result => {
        if(result == CpuPerformanceLevel.High){
            performanceInfo.level = CpuPerformanceLevel.High
        }
    })
}
