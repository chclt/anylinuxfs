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
 for(const char *name:{"www.txt","CHEN_5min_MIP04_Master_H264.mp4"}){auto id=find(downloads,name);auto info=s->GetObjectInfo(id);std::cout<<"EXISTING "<<name<<" raw="<<info.ModificationDate<<" epoch="<<mtp::ConvertDateTime(info.ModificationDate)<<std::endl;}
 check("device_datetime",[&]{std::cout<<s->GetDeviceStringProperty(mtp::DeviceProperty::Datetime)<<std::endl;});
 std::string dirname="alfs-time-study-"+std::to_string(time(nullptr));auto parent=s->CreateDirectory(dirname,mtp::Session::Root,storage).ObjectId;std::cout<<"CREATED_DIR "<<parent.Id<<" "<<dirname<<std::endl;
 for(const char *date:{"20200102T030405Z","20200102T110405"}){
  mtp::msg::ObjectInfo info;info.Filename=std::string(date)+".txt";info.ObjectFormat=mtp::ObjectFormat::Undefined;info.ObjectCompressedSize=4;info.ModificationDate=date;info.CaptureDate=date;
  auto id=s->SendObjectInfo(info,storage,parent).ObjectId;s->SendObject(std::make_shared<mtp::ByteArrayObjectInputStream>(mtp::ByteArray{1,2,3,4}));
  auto report=[&](const char *stage){auto x=s->GetObjectInfo(id);std::cout<<stage<<" id="<<id.Id<<" raw="<<x.ModificationDate<<" epoch="<<mtp::ConvertDateTime(x.ModificationDate)<<std::endl;};
  report("AFTER_CREATE");auto desc=s->GetObjectPropertyDesc(mtp::ObjectProperty::DateModified,info.ObjectFormat);std::cout<<"DATE_MODIFIED_GET_SET "<<unsigned(desc.at(4))<<std::endl;
  check("set_date_modified",[&]{s->SetObjectProperty(id,mtp::ObjectProperty::DateModified,std::string("20200203T040506Z"));});report("AFTER_SET");
  sleep(1);auto edit=mtp::Session::EditObject(s,id);edit->Send(0,mtp::ByteArray{5,6,7,8});edit->Finish();edit.reset();report("AFTER_EDIT");
  s->DeleteObject(id);std::cout<<"DELETED_FILE "<<id.Id<<std::endl;
 }
 s->DeleteObject(parent);std::cout<<"DELETED_DIR "<<parent.Id<<std::endl;s.reset();std::cout<<"CLOSED"<<std::endl;return 0;
}catch(const std::exception &e){std::cerr<<"FATAL "<<e.what()<<std::endl;return 1;}}
