#include <usb/Context.h>
#include <mtp/ptp/Device.h>
#include <mtp/ptp/Session.h>
#include <mtp/ptp/Messages.h>
#include <mtp/ptp/ByteArrayObjectStream.h>
#include <iostream>
#include <functional>
#include <unistd.h>
void check(const char *name,const std::function<void()> &fn){std::cout<<"BEGIN "<<name<<std::endl;try{fn();std::cout<<"OK "<<name<<std::endl;}catch(const std::exception &e){std::cout<<"ERROR "<<name<<" "<<e.what()<<std::endl;}}
int main(){try{
 auto context=std::make_shared<mtp::usb::Context>();auto device=mtp::Device::FindFirst(context);if(!device)throw std::runtime_error("No device");auto s=device->OpenSession(1);auto stores=s->GetStorageIDs().StorageIDs;if(stores.empty())throw std::runtime_error("No storage");auto storage=stores.front();
 auto find=[&](mtp::ObjectId parent,const std::string &name){for(auto id:s->GetObjectHandles(storage,mtp::ObjectFormat::Any,parent).ObjectHandles)if(s->GetObjectStringProperty(id,mtp::ObjectProperty::ObjectFilename)==name)return id;throw std::runtime_error("Missing "+name);};
 auto downloads=find(mtp::Session::Root,"Download");
 for(const char *name:{"www.txt","CHEN_5min_MIP04_Master_H264.mp4"}){
  auto id=find(downloads,name);auto info=s->GetObjectInfo(id);
  std::cout<<"FILE "<<name<<" id="<<id.Id<<" capture="<<info.CaptureDate<<" modified="<<info.ModificationDate<<std::endl;
  check("date_added",[&]{auto v=s->GetObjectStringProperty(id,mtp::ObjectProperty::DateAdded);std::cout<<"raw="<<v<<" epoch="<<mtp::ConvertDateTime(v)<<std::endl;});
  check("date_created",[&]{auto v=s->GetObjectStringProperty(id,static_cast<mtp::ObjectProperty>(0xdc08));std::cout<<"raw="<<v<<" epoch="<<mtp::ConvertDateTime(v)<<std::endl;});
 }
 s.reset();std::cout<<"CLOSED"<<std::endl;return 0;
}catch(const std::exception &e){std::cerr<<"FATAL "<<e.what()<<std::endl;return 1;}}
