// UploadManager.ts
import {HashWorkerManager} from './hash-worker-manager';

interface Chunk {
    status: 'waiting' | 'uploading' | 'completed';
    chunkNumber: number;
    chunkSize: number;
    chunkHash: string | null;
    chunkData: Blob;
}

export enum UploadState {
    New = 'new' ,
    Initialized = 'initialized' ,
    Uploading = 'uploading' ,
    Paused = 'paused' ,
    Completed = 'completed' ,
    Error = 'error',
}

export interface UploadStatus {
    fileId: string;
    status: UploadState;
    progress: number;
    uploadedChunks: number[];
    uploadedSize: number;
    totalSize: number;
    error?: string;
}

export function newUploadStatus(file: File): UploadStatus{
    return {
        fileId: '', // 这是后端的upload_id，initialized之后才会有
        status: UploadState.New,
        progress: 0,
        uploadedChunks: [],
        uploadedSize: 0,
        totalSize: file.size,
    }
}

export interface InitUploadResponse {
    file_id: string;
    status: 'Uploading' | 'Completed';
    uploaded_chunks: number[];
    uploaded_size: number;
}

export interface UploadOptions {
    chunkSize: number;
    maxConcurrentUploads: number;
    onProgress: (status: UploadStatus) => void;
    onError: (error: string) => void;
    onComplete: () => void;
}

// 默认配置
const DEFAULT_OPTIONS: UploadOptions = {
    chunkSize: 5 * 1024 * 1024, // 5MB
    maxConcurrentUploads: 3, onProgress: (status: UploadStatus) => {
        console.log(status)
    }, onError: (error_msg: string) => {
        console.error(error_msg);
    }, onComplete: () => {
        console.log('Complete')
    }
};

// 上传管理器类（用于对单个文件进行分片上传）
export class UploadManager {
    private options: UploadOptions;
    private file: File | null = null;
    private fileHash: string | null = null;
    private status: UploadStatus | null = null;
    private isPaused: boolean = false;
    private abortControllers: Map<number, AbortController> = new Map();
    private pendingChunks: Chunk[] = [];

    constructor(options: Partial<UploadOptions> = {}) {
        this.options = {...DEFAULT_OPTIONS, ...options};
    }

    // 设置文件
    async setFile(file: File): Promise<boolean> {
        if(this.file && this.file !== file){
            throw new Error('File already exist');
        }
        this.file = file;
        if(this.status === null) {
            this.status = newUploadStatus(file);
        }

        try {
            if(this.fileHash === null){
                // 使用Web Worker计算文件哈希
                console.log(`[UploadManager::setFile]开始计算 ${file.name} 文件的哈希值...`);
                const hashWorkerManager = new HashWorkerManager();
                this.fileHash = await hashWorkerManager.calculateHash(file);
                console.log(`[UploadManager::setFile]${file.name} 文件的哈希值: ${this.fileHash}`);
            }

            // 如果在计算哈希的过程中用户暂停了上传，那么退出上传
            if(this.isPaused) {
                console.log(`[UploadManager::setFile]用户已暂停上传[${file.name}]，退出...`);
                return false;
            }

            // 初始化上传
            const initResponse = await this.initUpload(file, this.fileHash);

            // 如果在计算哈希的过程中用户暂停了上传，那么退出上传
            if(this.isPaused) {
                console.log(`[UploadManager::setFile]用户已暂停上传[${file.name}]，退出...`);
                return false;
            }

            this.updateStatus({
                fileId: initResponse.file_id,
                status: initResponse.status === 'Completed' ? UploadState.Completed : UploadState.Initialized,
                progress: initResponse.uploaded_size / file.size * 100,
                uploadedChunks: initResponse.uploaded_chunks,
                uploadedSize: initResponse.uploaded_size,
                totalSize: file.size,
            });
            return true;
        } catch (error) {
            console.error('[UploadManager::setFile]出现异常', error);
            this.handleError(error);
            throw error;
        }
    }

    slice(chunkNumber: number): Chunk {
        const start = (chunkNumber - 1) * this.options.chunkSize;
        const end = Math.min(start + this.options.chunkSize, this.file!.size);
        const blob = this.file!.slice(start, end);
        return {
            status: 'waiting', chunkNumber, chunkSize: end - start, chunkHash: null, chunkData: blob,
        };
    }

    // 开始/恢复上传
    async startOrResumeUpload(): Promise<void> {
        if (!this.file || !this.status) {
            throw new Error('没有可用的文件或状态');
        }

        if(this.status.status === UploadState.New){
            throw new Error('未初始化');
        }

        if (this.status.status === UploadState.Completed) {
            console.log('[UploadManager::startOrResumeUpload]文件此前已经上传完成，跳过');
            return;
        }

        if (this.status.status === UploadState.Uploading) {
            console.log('[UploadManager::startOrResumeUpload]文件正在上传中，不要重复调用startOrResumeUpload');
            return;
        }

        this.isPaused = false;
        this.updateStatus({...this.status, status: UploadState.Uploading});

        const fileId = this.status.fileId;
        const totalChunks = Math.ceil(this.file.size / this.options.chunkSize);

        // 确定需要上传的分片
        const chunksToUpload: number[] = [];
        for (let i = 1; i <= totalChunks; i++) {
            if (!this.status.uploadedChunks.includes(i)) {
                chunksToUpload.push(i);
            }
        }

        console.log('[UploadManager::startOrResumeUpload]【%s】文件待上传的分片：', this.file.name, chunksToUpload);

        if (this.pendingChunks.length === 0) {
            const chunks: Chunk[] = [];
            for (const chunkNumber of chunksToUpload) {
                const chunk: Chunk = this.slice(chunkNumber);
                chunks.push(chunk);
            }
            this.pendingChunks = chunks;
        }

        // 并发上传控制
        const uploadNextChunk = async (): Promise<void> => {
            if (this.isPaused) return;
            const chunks = this.pendingChunks
                .filter((chunk: Chunk) => chunk.status === 'waiting');
            if (chunks.length === 0) {
                console.log('[UploadManager::startOrResumeUpload::uploadNextChunk]没有waiting状态的分片');
                return Promise.resolve();
            }
            const chunk = chunks[0];
            chunk.status = 'uploading';

            const chunkNumber = chunk.chunkNumber;
            const blob = chunk.chunkData;

            if (chunk.chunkHash === null) {
                // 使用Web Worker计算分片哈希
                console.log(`[UploadManager::startOrResumeUpload::uploadNextChunk]开始计算分片${chunkNumber}的哈希`);
                const hashWorkerManager = new HashWorkerManager();
                chunk.chunkHash = await hashWorkerManager.calculateHash(blob);
                console.log(`[UploadManager::startOrResumeUpload::uploadNextChunk]分片${chunkNumber}的哈希值是：${chunk.chunkHash}`);
            }

            // 如果在计算哈希的过程中用户暂停了上传，那么退出上传
            if(this.isPaused) {
                console.log(`[UploadManager::startOrResumeUpload::uploadNextChunk]用户暂停了上传[${this.file!.name}]，退出...`);
                return;
            }

            await this.doUploadChunk(fileId, chunkNumber, blob, chunk.chunkHash);

            const index = this.pendingChunks.indexOf(chunk);
            this.pendingChunks.splice(index, 1);

            return uploadNextChunk();
        };

        // 启动并发上传
        const concurrentUploads = Math.min(this.options.maxConcurrentUploads, chunksToUpload.length);
        const uploadPromises: Promise<void>[] = [];

        for (let i = 0; i < concurrentUploads; i++) {
            uploadPromises.push(uploadNextChunk());
        }

        try {
            await Promise.all(uploadPromises);

            console.log('[UploadManager::startOrResumeUpload]所有分片已经上传完成，开始发送合并请求');
            await this.completeUpload(fileId);

        } catch (error) {
            this.handleError(error);
            throw error;
        }
    }

    // 暂停上传
    pauseUpload(): void {
        // 取消所有进行中的上传
        this.abortControllers.forEach((controller, chunkNumber) => {
            controller.abort();
            this.abortControllers.delete(chunkNumber);
        });

        this.isPaused = true;
        this.updateStatus({...this.status!, status: UploadState.Paused});
    }

    resumeUpload(): void {
        if(this.isPaused){
            this.isPaused = false;
            this.pendingChunks.forEach((chunk: Chunk) => {
                if(chunk.status === 'uploading'){
                    chunk.status = 'waiting';
                }
            })
        }
    }

    // 取消上传
    cancelUpload(): void {
        this.pauseUpload();
        this.status = null;
        this.file = null;
        this.pendingChunks = [];
    }

    // 获取当前状态
    getStatus(): UploadStatus | null {
        return this.status;
    }

    // 初始化上传
    private async initUpload(file: File, fileHash: string): Promise<InitUploadResponse> {
        const response = await fetch(`/upload/init`, {
            method: 'POST', headers: {
                'Content-Type': 'application/json',
            }, body: JSON.stringify({
                file_name: file.name, file_size: file.size, file_hash: fileHash,
            }),
        });

        if (!response.ok) {
            const responseText = await response.text();
            throw new Error(`初始化上传失败, statusText: ${response.statusText}, responseText: ${responseText}`);
        }

        return await response.json();
    }

    // 上传单个分片
    private async doUploadChunk(fileId: string, chunkNumber: number, chunk: Blob, chunkHash: string): Promise<void> {
        const jsonData = {
            file_id: fileId, chunk_number: chunkNumber, chunk_hash: chunkHash,
        };

        const jsonBlob = new Blob([JSON.stringify(jsonData)], {type: 'application/json'});

        const formData = new FormData();
        formData.append('json', jsonBlob);
        formData.append('file', chunk);

        const controller = new AbortController();
        this.abortControllers.set(chunkNumber, controller);

        try {
            const response = await fetch(`/upload/chunk`, {
                method: 'POST', body: formData, signal: controller.signal,
            });

            if (!response.ok) {
                const responseText = await response.text();
                if (responseText === 'File already completed') {
                    console.warn('File already completed');
                    this.status!.status = UploadState.Completed;
                } else {
                    throw new Error(`分片上传失败: ${responseText}`);
                }
            }

            // 更新上传状态
            this.updateStatus({
                ...this.status!,
                uploadedChunks: [...this.status!.uploadedChunks, chunkNumber],
                uploadedSize: this.status!.uploadedSize + chunk.size,
                progress: (this.status!.uploadedSize + chunk.size) / this.status!.totalSize * 100,
            });
        } finally {
            this.abortControllers.delete(chunkNumber);
        }
    }

    // 完成上传
    private async completeUpload(fileId: string): Promise<void> {
        const response = await fetch(`/upload/complete`, {
            method: 'POST', headers: {
                'Content-Type': 'application/json',
            }, body: JSON.stringify({
                file_id: fileId,
            }),
        });

        if (!response.ok) {
            throw new Error(`完成上传失败: ${response.statusText}`);
        }

        this.updateStatus({...this.status!, status: UploadState.Completed});

        if (this.options.onComplete) {
            this.options.onComplete();
        }
    }

    // 更新状态
    private updateStatus(newStatus: UploadStatus): void {
        this.status = newStatus;

        if (this.options.onProgress) {
            this.options.onProgress(newStatus);
        }

        if (newStatus.status === 'completed' && this.options.onComplete) {
            this.options.onComplete();
        }
    }

    // 处理错误
    private handleError(error: unknown): void {
        if (error instanceof DOMException && error.name === 'AbortError') {
            if (this.status) {
                this.updateStatus({...this.status, status: UploadState.Paused});
            }
        } else {
            if (this.status) {
                this.updateStatus({...this.status, status: UploadState.Error, error: JSON.stringify(error)});
            }
            if (this.options.onError) {
                this.options.onError(JSON.stringify(error));
            }
        }
    }
}

