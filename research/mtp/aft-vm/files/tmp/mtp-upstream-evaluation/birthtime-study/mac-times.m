#import <Foundation/Foundation.h>
#include <sys/attr.h>
#include <sys/stat.h>
#include <sys/xattr.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
int main(int argc,char **argv){@autoreleasepool{
 for(int n=1;n<argc;++n){const char *p=argv[n];struct stat st={0};errno=0;int r=stat(p,&st);printf("PATH %s\nstat rc=%d errno=%d birth=%lld mtime=%lld ctime=%lld\n",p,r,errno,(long long)st.st_birthtimespec.tv_sec,(long long)st.st_mtimespec.tv_sec,(long long)st.st_ctimespec.tv_sec);
 struct attrlist attrs={.bitmapcount=ATTR_BIT_MAP_COUNT,.commonattr=ATTR_CMN_CRTIME|ATTR_CMN_MODTIME|ATTR_CMN_CHGTIME|ATTR_CMN_RETURNED_ATTRS};
 unsigned char buffer[256]={0};errno=0;r=getattrlist(p,&attrs,buffer,sizeof(buffer),FSOPT_PACK_INVAL_ATTRS);
 uint32_t len=0;attribute_set_t returned={0};struct timespec times[3]={0};memcpy(&len,buffer,4);memcpy(&returned,buffer+4,sizeof(returned));memcpy(times,buffer+4+sizeof(returned),sizeof(times));
 printf("getattrlist rc=%d errno=%d len=%u returned_common=%x birth=%lld mtime=%lld ctime=%lld\n",r,errno,len,returned.commonattr,(long long)times[0].tv_sec,(long long)times[1].tv_sec,(long long)times[2].tv_sec);
 NSString *path=[NSString stringWithUTF8String:p];NSError *err=nil;NSDictionary *a=[[NSFileManager defaultManager]attributesOfItemAtPath:path error:&err];NSDate *created=a[NSFileCreationDate];printf("NSFileManager creation=%s unix=%.0f reference=%.0f error=%s\n",created.description.UTF8String,created.timeIntervalSince1970,created.timeIntervalSinceReferenceDate,err.description.UTF8String);
 NSURL *url=[NSURL fileURLWithPath:path];NSDate *date=nil;err=nil;BOOL ok=[url getResourceValue:&date forKey:NSURLCreationDateKey error:&err];printf("NSURL creation ok=%d value=%s unix=%.0f reference=%.0f error=%s\n",ok,date.description.UTF8String,date.timeIntervalSince1970,date.timeIntervalSinceReferenceDate,err.description.UTF8String);
 char list[4096]={0};ssize_t size=listxattr(p,list,sizeof(list),0);printf("xattr_list_size=%zd errno=%d\n",size,errno);for(ssize_t off=0;off<size;off+=strlen(list+off)+1)printf("xattr %s\n",list+off);
 }
}return 0;}
