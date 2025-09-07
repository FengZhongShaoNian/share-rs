import {newUploadStatus, UploadManager, UploadOptions, UploadState, UploadStatus} from "@/app/api/upload-manager";

export interface BatchUploadFile {
    id: string;
    file: File;
    status: UploadStatus;
    uploadManager: UploadManager | null;
    uploadOptions?: Partial<UploadOptions>;
}

export interface BatchUploadOptions {
    maxConcurrentFiles: number;
    onFileProgress: (fileId: string, status: UploadStatus) => void;
}

export class BatchUploadManager {
    private files: Map<string, BatchUploadFile> = new Map();
    private activeUploads: Set<string> = new Set();
    private options: Required<BatchUploadOptions>;
    private totalSize: number = 0;
    private uploadedSize: number = 0;

    constructor(options: Partial<BatchUploadOptions> = {}) {
        this.options = {
            maxConcurrentFiles: 1,
            onFileProgress: () => {
            },
            ...options,
        };
    }

    // 添加文件到批量上传
    addFile(file: File, uploadOptions?: Partial<UploadOptions>): string {
        const id = this.generateFileId(file);

        if (this.files.has(id)) {
            throw new Error('文件已存在');
        }

        const status: UploadStatus = newUploadStatus(file);
        const batchFile: BatchUploadFile = {
            id,
            file,
            status,
            uploadManager: null,
            uploadOptions
        };

        this.files.set(id, batchFile);
        this.totalSize += file.size;

        return id;
    }

    // 移除文件
    removeFile(fileId: string): void {
        const file = this.files.get(fileId);
        if (!file) return;

        if (this.activeUploads.has(fileId)) {
            file.uploadManager!.pauseUpload();
            this.activeUploads.delete(fileId);
        }

        this.totalSize -= file.file.size;
        this.uploadedSize -= file.status?.uploadedSize || 0;
        this.files.delete(fileId);
    }

    // 开始批量上传
    async startBatchUpload(): Promise<void> {
        await this.processQueue();
    }

    async startUpload(fileId: string): Promise<void> {
        const file = this.files.get(fileId);
        if (!file) return;
        console.log(`[BatchUploadManager::startUpload] fileId: ${fileId}, status: ${file.status?.status}`);
        if(file.status?.status === 'paused' || file.status?.status === 'error') {
            console.log(`[BatchUploadManager::startUpload] 将 ${fileId} 从 ${file.status?.status} 状态更新为 new 状态`);
            file.status.status = UploadState.New;
            file.uploadManager?.resumeUpload();
            this.handleFileProgress(fileId, file.status);
        }

        await this.processQueue();
    }

    async pauseUpload(fileId: string): Promise<void> {
        const file = this.files.get(fileId);
        if (!file) return;
        if(file.status?.status === UploadState.New || file.status?.status === UploadState.Uploading){
            this.activeUploads.delete(fileId);
            if(file.uploadManager){
                file.uploadManager.pauseUpload();
            }else{
                file.status.status = UploadState.Paused;
                this.handleFileProgress(fileId, file.status);
            }
        }
        await this.processQueue();
    }

    // 处理文件进度更新
    private handleFileProgress(fileId: string, status: UploadStatus): void {
        console.log('[BatchUploadManager] handleFileProgress, fileId:%s, status: %s, progress: %d', fileId, status.status, status.progress);
        const file = this.files.get(fileId);
        if (!file) return;

        // 更新已上传大小
        const previousSize = file.status?.uploadedSize || 0;
        this.uploadedSize += status.uploadedSize - previousSize;

        file.status = status;

        // 通知文件进度
        this.options.onFileProgress(fileId, status);
    }

    // 处理文件完成
    private handleFileComplete(fileId: string): void {
        const file = this.files.get(fileId);
        if (!file) return;

        this.activeUploads.delete(fileId);
    }

    // 处理文件错误
    private handleFileError(fileId: string, error: string): void {
        console.error(error);
        const file = this.files.get(fileId);
        if (!file) return;

        this.activeUploads.delete(fileId);
    }

    // 处理上传队列
    private async processQueue(): Promise<void> {
        // 如果已达最大并发数，则等待
        if (this.activeUploads.size >= this.options.maxConcurrentFiles) {
            return;
        }

        // 查找下一个等待上传的文件
        const nextFile = Array.from(this.files.values()).find(file => {
            const isNotActive = !this.activeUploads.has(file.id);
            const isPending = file.status?.status === UploadState.New || file.status?.status === UploadState.Initialized;

            return isNotActive && isPending;
        });

        if (!nextFile) {
            // 没有更多文件需要上传
            if (this.activeUploads.size === 0) {
                console.log('没有更多文件需要上传');
            }
            return;
        }

        if(nextFile.uploadManager === null) {
            nextFile.uploadManager = new UploadManager({
                ...nextFile.uploadOptions,
                onProgress: (status) => {
                    this.handleFileProgress(nextFile.id, status);
                },
                onComplete: () => {
                    this.handleFileComplete(nextFile.id);
                },
                onError: (error) => {
                    this.handleFileError(nextFile.id, error);
                },
            });
        }

        // 开始上传文件
        this.activeUploads.add(nextFile.id);

        try {
            const isContinue = await nextFile.uploadManager.setFile(nextFile.file);
            if(!isContinue) {
                console.log('[BatchUploadManager::processQueue]uploadManage.setFile返回false，退出[%s]文件的上传...', nextFile.file.name);
                return;
            }
            if(!this.activeUploads.has(nextFile.id)) {
                console.log('[BatchUploadManager::processQueue]用户已暂停[%s]文件的上传，退出...', nextFile.file.name);
            }else {
                await nextFile.uploadManager.startOrResumeUpload();
            }
        } catch (error) {
            console.error(`文件 ${nextFile.id} 上传失败:`, error);
            this.activeUploads.delete(nextFile.id);
        }

        // 递归处理队列，直到达到最大并发数
        await this.processQueue();
    }

    // 生成文件ID
    private generateFileId(file: File): string {
        return `${file.name}-${file.size}-${file.lastModified}-${Math.random().toString(36).substring(2, 9)}`;
    }
}