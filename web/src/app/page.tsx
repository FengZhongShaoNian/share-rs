'use client'

import {useEffect, useState, MouseEvent} from "react";

import {getShareList, ShareList} from "@/app/api/request";
import {ThemeProvider} from "next-themes";
import DownloadIcon from '@/app/icon/download.svg';
import HeaderBar from "@/component/header-bar";

export default function Home() {

    const emptyList: ShareList = []
    const [shareList, setShareList] = useState(emptyList);

    useEffect(() => {
        getShareList().then(data => {
            setShareList(data)
        }).catch(console.error)
    }, []);

    const preview = (fileId: string) => {
        window.open(`/stream/${fileId}?force_download=false`, "_blank");
    };
    const download = (event: MouseEvent, fileId: string) => {
        event.nativeEvent.stopPropagation();
        window.open(`/stream/${fileId}?force_download=true`, "_blank");
    };
    return (
        <ThemeProvider>
            <div className="flex flex-col h-full justify-start">
                <HeaderBar pageTitle={"Share"}/>
                <div className="h-full flex flex-col justify-start">
                    {
                        shareList.map((shareItem, index) => {
                            return (
                                <div
                                    className="h-12 pl-4 pr-4 flex flex-row justify-start items-center gap-4 hover:bg-blue-100 dark:hover:bg-custom-gray"
                                    key={index} onClick={() => preview(shareItem.id)}>
                                    <img className="size-8" src={"/icons?mime_type=" + encodeURIComponent(shareItem.mime_type)}
                                         alt="File Icon"/>
                                    <div className="grow shrink truncate">{shareItem.file_name}</div>
                                    <div className="size-8" onClick={(event) => download(event, shareItem.id)}>
                                        <DownloadIcon className="dark:fill-white hover:fill-blue-400 cursor-pointer active:fill-blue-200"/>
                                    </div>

                                </div>
                            )
                        })
                    }
                </div>
            </div>

        </ThemeProvider>
    );
}
