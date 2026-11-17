// Read-only probe: query the same MTP session repeatedly without FUSE/NFS caches.
#include <usb/Context.h>
#include <mtp/ptp/Device.h>
#include <mtp/ptp/Session.h>
#include <mtp/ptp/Messages.h>
#include <iostream>
#include <chrono>
#include <thread>
#include <fstream>
int main(int argc,char **argv) {
 try {
  mtp::usb::ContextPtr context(new mtp::usb::Context);
  auto device=mtp::Device::FindFirst(context);
  if(!device)throw std::runtime_error("No MTP device");
  auto session=device->OpenSession(1);auto stores=session->GetStorageIDs().StorageIDs;
  if(stores.empty())throw std::runtime_error("No MTP storage");
  auto storage=stores.front();auto dir=mtp::Session::Root;bool found=false;
  for(auto id:session->GetObjectHandles(storage,mtp::ObjectFormat::Any,mtp::Session::Root).ObjectHandles)
   if(session->GetObjectStringProperty(id,mtp::ObjectProperty::ObjectFilename)=="Download"){dir=id;found=true;break;}
  if(!found)throw std::runtime_error("Download missing");
  for(int n=0;n<450;++n) {
   if(argc>1 && std::ifstream(argv[1]).good())break;
   auto begin=std::chrono::steady_clock::now();
   auto info=session->GetObjectInfo(dir);auto handles=session->GetObjectHandles(storage,mtp::ObjectFormat::Any,dir).ObjectHandles;
   std::cout<<"SCAN "<<n<<" time="<<time(nullptr)<<" directory="<<dir.Id<<" modified="<<info.ModificationDate<<" count="<<handles.size()<<std::endl;
   for(auto id:handles){auto name=session->GetObjectStringProperty(id,mtp::ObjectProperty::ObjectFilename);
    if(name.find("www")!=std::string::npos || name.find("alfs-refresh")!=std::string::npos){auto file=session->GetObjectInfo(id);std::cout<<"MATCH "<<id.Id<<" "<<name<<" size="<<file.ObjectCompressedSize<<" modified="<<file.ModificationDate<<std::endl;}}
   std::cout<<"DONE "<<n<<" seconds="<<std::chrono::duration<double>(std::chrono::steady_clock::now()-begin).count()<<std::endl;
   std::this_thread::sleep_for(std::chrono::seconds(2));
  }
  session.reset();std::cout<<"CLOSED"<<std::endl;return 0;
 }catch(const std::exception &e){std::cerr<<"ERROR "<<e.what()<<std::endl;return 1;}
}
