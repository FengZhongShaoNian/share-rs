import ShareIcon from "@/app/icon/share.svg";
import UploadIcon from "@/app/icon/upload.svg";

export default function HeaderBar(props: {pageTitle: string}){
    const goToSharePage = () => {
        window.location.href = '/web/index.html';
    }
    const goToUploadPage = () => {
        window.location.href = '/web/upload.html';
    }

    return (
        <div
            className="h-12 flex flex-row justify-between items-center bg-header-bar border-header-bar-border border-b-2 shadow-md pl-4 pr-4">
            <div onClick={goToSharePage} title={"To share page"}>
                <ShareIcon
                    className="w-8 h-8 fill-white active:fill-custom-gray hover:fill-blue-400 cursor-pointer"/>
            </div>
            <div>{props.pageTitle}</div>
            <div onClick={goToUploadPage} title={"To upload page"}>
                <UploadIcon
                    className="w-8 h-8 fill-white active:fill-custom-gray hover:fill-blue-400 cursor-pointer"/>
            </div>
        </div>
    )
}