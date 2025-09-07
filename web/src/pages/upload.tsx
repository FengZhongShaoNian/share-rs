'use client'
import "@/app/globals.css";
import HeaderBar from "@/component/header-bar";
import {ThemeProvider} from "next-themes";
import PauseIcon from "@/app/icon/pause.svg";
import StartIcon from "@/app/icon/start.svg";
import AbortIcon from "@/app/icon/abort.svg";
import {useEffect, useRef, useState} from "react";
import {BatchUploadManager} from "@/app/api/batch-upload-manager";
import {UploadState, UploadStatus} from "@/app/api/upload-manager";
import {LinearProgress} from "@mui/material";

enum UploadItemStatus {
    Waiting = '排队中', Uploading = '上传中', Paused = '暂停', Completed = '已完成',
}

interface UploadItem {
    itemId?: string,
    fileName: string,
    fileSize: number,
    uploadedSize: number,
    status: UploadItemStatus,
    progress: number,
    mimeType: string,
    error?: string,
    // 一些字段用于控制条件渲染
    canStart?: boolean,
    canPause?: boolean,
}

function convertStatus(status: UploadState): UploadItemStatus {
    switch (status) {
        case UploadState.New:
            return UploadItemStatus.Waiting;
        case UploadState.Initialized:
            return UploadItemStatus.Uploading;
        case UploadState.Uploading:
            return UploadItemStatus.Uploading;
        case UploadState.Paused:
            return UploadItemStatus.Paused;
        case UploadState.Error:
            return UploadItemStatus.Paused;
        case UploadState.Completed:
            return UploadItemStatus.Completed;

    }
}

function formatBytes(bytes: number, decimals = 2) {
    if (bytes === 0) return '0 B';

    const k = 1024;
    // 保留的小数位数（dm）
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];

    // 计算应该使用哪个单位：通过计算字节数以1024为底的对数，然后取整得到索引i。
    // 例如：如果bytes是2048，那么Math.log(2048)/Math.log(1024)等于1，i=1，对应的单位是KB。
    const i = Math.floor(Math.log(bytes) / Math.log(k));

    return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

export default function Upload() {
    const emptyList: Array<UploadItem> = [];
    const [uploadList, setUploadList] = useState(emptyList);

    // 使用useRef来保存BatchUploadManager实例
    const batchUploadManagerRef = useRef<BatchUploadManager | null>(null);

    // 使用useRef来跟踪最新的uploadList
    const uploadListRef = useRef<Array<UploadItem>>([]);
    uploadListRef.current = uploadList;

    // 初始化BatchUploadManager
    useEffect(() => {
        batchUploadManagerRef.current = new BatchUploadManager({
            onFileProgress: (fileId: string, status: UploadStatus) => {
                // 使用函数式更新确保基于最新状态
                setUploadList(prevList => {
                    return prevList.map(item => {
                        if (item.itemId === fileId) {
                            const sts = convertStatus(status.status);
                            return {
                                ...item,
                                status: sts,
                                progress: status.progress,
                                uploadedSize: status.uploadedSize,
                                canStart: sts === UploadItemStatus.Paused,
                                canPause: sts === UploadItemStatus.Waiting || sts === UploadItemStatus.Uploading,
                            };
                        }
                        return item;
                    });
                });
            }
        });

        // 清理函数
        return () => {
            // 必要的清理操作
        };
    }, []); // 空依赖数组，只运行一次

    const addUploadTasks = (files: FileList) => {
        console.log('files:', files);

        if (!batchUploadManagerRef.current) return;

        const newItems: UploadItem[] = [];

        // 先创建所有新项目
        for (const file of files) {
            const uploadItem: UploadItem = {
                fileName: file.name,
                fileSize: file.size,
                uploadedSize: 0,
                progress: 0,
                status: UploadItemStatus.Waiting,
                mimeType: file.type,
                itemId: batchUploadManagerRef.current!.addFile(file),
                canStart: false,
                canPause: true,
            };
            newItems.push(uploadItem);
        }

        // 一次性更新状态
        setUploadList(prevList => [...prevList, ...newItems]);

        // 开始上传
        batchUploadManagerRef.current.startBatchUpload().then(() => {
            console.log('开始上传');
        });
    }

    const selectFiles = () => {
        const input = document.createElement('input');
        input.type = 'file'
        input.multiple = true;
        input.addEventListener('change', () => {
            if (input.files != null) {
                addUploadTasks(input.files);
            }
        });
        input.click();
    };

    const startUpload = (uploadItem: UploadItem) => {
        if (uploadItem.itemId) {
            batchUploadManagerRef.current?.startUpload(uploadItem.itemId);
        }
    }
    const pauseUpload = (uploadItem: UploadItem) => {
        if (uploadItem.itemId) {
            batchUploadManagerRef.current?.pauseUpload(uploadItem.itemId);
        }
    }

    const abortUpload = (uploadItem: UploadItem) => {
        setUploadList(prevList => {
            return prevList.filter(item => {
                return item.itemId != uploadItem.itemId
            });
        });

        if (uploadItem.itemId) {
            batchUploadManagerRef.current?.removeFile(uploadItem.itemId);
        }
    }

    return (<ThemeProvider>
        <div className="flex flex-col h-full justify-start gap-4">
            <HeaderBar pageTitle={"Upload"}/>
            <div className="h-full flex flex-col justify-start">
                <div onClick={selectFiles}
                     className="h-14 cursor-pointer border-2 border-dashed flex flex-row justify-center items-center ml-4 mr-4 hover:text-blue-400 hover:border-blue-400">
                    <div className="font-bold ">点击上传文件</div>
                </div>
            </div>
            <div className="h-full flex flex-col justify-start">
                {uploadList.map((uploadItem, index) => {
                    return (
                        <div key={index}
                             className="pt-2 pb-2 flex flex-col justify-start gap-1 hover:bg-blue-100 dark:hover:bg-custom-gray">
                            <div
                                className="h-12 pl-4 pr-4 flex flex-row justify-start items-center gap-4 "
                            >
                                <img className="size-8"
                                     src={"/icons?mime_type=" + encodeURIComponent(uploadItem.mimeType)}
                                     alt="File Icon"/>
                                <div className="grow shrink truncate">{uploadItem.fileName}</div>

                                {uploadItem.canStart ? (
                                    <div className="size-8" onClick={() => startUpload(uploadItem)}>
                                        <StartIcon
                                            className="dark:fill-white active:fill-custom-gray hover:fill-blue-400 cursor-pointer"/>
                                    </div>
                                ) : null}

                                {uploadItem.canPause ? (
                                    <div className="size-8" onClick={() => pauseUpload(uploadItem)}>
                                        <PauseIcon
                                            className="dark:fill-white active:fill-custom-gray hover:fill-blue-400 cursor-pointer"/>
                                    </div>
                                ) : null}

                                <div className="size-8" onClick={() => abortUpload(uploadItem)}>
                                    <AbortIcon
                                        className="dark:fill-white active:fill-custom-gray hover:fill-blue-400 cursor-pointer"/>
                                </div>
                            </div>
                            <div className="pl-16 pr-16">
                                <LinearProgress variant="determinate" value={uploadItem.progress}/>
                            </div>
                            <div className="pl-16 pr-16 flex flex-row justify-start justify-items-center gap-4 text-xs">
                                <div className="grow">
                                    {formatBytes(uploadItem.uploadedSize, 1)} / {formatBytes(uploadItem.fileSize, 1)}
                                </div>

                                <div className="flex-none">状态：{uploadItem.status}</div>
                            </div>
                        </div>
                    )
                })}
            </div>
        </div>

    </ThemeProvider>);
}